/* rusty-ircd - an IRC daemon written in Rust
*  Copyright (C) Joanna Janet Zaitseva-Doyle <jjadoyle@gmail.com>

*  This program is free software: you can redistribute it and/or modify
*  it under the terms of the GNU Lesser General Public License as
*  published by the Free Software Foundation, either version 3 of the
*  License, or (at your option) any later version.

*  This program is distributed in the hope that it will be useful,
*  but WITHOUT ANY WARRANTY; without even the implied warranty of
*  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
*  GNU Lesser General Public License for more details.

*  You should have received a copy of the GNU Lesser General Public License
*  along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
extern crate tokio;
extern crate log;
use crate::irc::chan::{Channel, ChanError};
use crate::irc::error::Error as ircError;
use crate::irc::reply::Reply as ircReply;
use crate::irc::{self, Core, User, NamedEntity};
use crate::parser::{parse_message, ParseError};
use std::error;
use std::fmt;
use std::io::Error as ioError;
use std::net::IpAddr;
use std::sync::{Arc, Weak, Mutex};
use log::{debug, warn};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError as mpscSendErr;

/* There are 3 main types of errors we can have here...
 * one is a parsing error, which should be covered by ParseError,
 * another important type is any other IRC error associated to
 * a particular command. There is a little overlap in the RFC
 * with IRC and parsing errors, but in this program the distinction
 * is what bit of code generates them.
 * The third main type will be related to the client connection or
 * system IO */
#[derive(Debug)]
pub enum GenError {
    Io(ioError),
    Parse(ParseError),
    IRC(ircError),
    Mpsc(mpscSendErr<String>),
    Chan(ChanError),
    DeadClient(Arc<User>, Vec<Arc<Channel>>),
    DeadUser(String),
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenError::Io(ref err) => write!(f, "IO Error: {}", err),
            GenError::Parse(ref err) => write!(f, "Parse Error: {}", err),
            GenError::IRC(ref err) => write!(f, "IRC Error: {}", err),
            GenError::Mpsc(ref err) => write!(f, "MPSC Send Error: {}", err),
            GenError::Chan(ref err) => write!(f, "Channel Error: {}", err),
            GenError::DeadClient(user, _chans) => write!(f, "user {}, stale client", user.get_nick()),
            GenError::DeadUser(nick) => write!(f, "user {}, remant, scattered WeakRefs", nick),
        }
    }
}

impl error::Error for GenError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            // N.B. Both of these implicitly cast `err` from their concrete
            // types (either `&io::Error` or `&num::ParseIntError`)
            // to a trait object `&Error`. This works because both error types
            // implement `Error`.
            GenError::Io(ref err) => Some(err),
            GenError::Parse(ref err) => Some(err),
            GenError::IRC(ref err) => Some(err),
            GenError::Mpsc(ref err) => Some(err),
            GenError::DeadClient(_user, _chans) => None,
            GenError::DeadUser(_nick) => None,
            GenError::Chan(ref err) => Some(err),
        }
    }
}

impl From<ioError> for GenError {
    fn from(err: ioError) -> GenError {
        GenError::Io(err)
    }
}

impl From<ParseError> for GenError {
    fn from(err: ParseError) -> GenError {
        GenError::Parse(err)
    }
}

impl From<ChanError> for GenError {
    fn from(err: ChanError) -> GenError {
        GenError::Chan(err)
    }
}

impl From<ircError> for GenError {
    fn from(err: ircError) -> GenError {
        GenError::IRC(err)
    }
}

impl From<mpscSendErr<String>> for GenError {
    fn from(err: mpscSendErr<String>) -> GenError {
        GenError::Mpsc(err)
    }
}

#[derive(Debug)]
pub enum Host {
    Hostname(String),
    HostAddr(IpAddr),
}

impl Clone for Host {
    fn clone(&self) -> Self {
        match &self {
            Host::Hostname(host) => Host::Hostname(host.clone()),
            Host::HostAddr(ip) => Host::HostAddr(*ip),
        }
    }
}

pub fn create_host_string(host_var: &Host) -> String {
    match host_var {
        Host::Hostname(hostname_str) => hostname_str.to_string(),
        Host::HostAddr(ip_addr) => ip_addr.to_string(),
    }
}

#[derive(Debug)]
pub enum ClientType {
    Dead,
    Unregistered,
    User(Arc<irc::User>),
    //Server(Arc<Mutex<irc::Server>>), leave serv implmentation for much later
    ProtoUser(Arc<Mutex<irc::ProtoUser>>),
}

impl Clone for ClientType {
    fn clone(&self) -> Self {
        match self {
            ClientType::Dead => ClientType::Dead,
            ClientType::Unregistered => ClientType::Unregistered,
            ClientType::User(user_ptr) => ClientType::User(Arc::clone(user_ptr)),
            ClientType::ProtoUser(proto_user_ptr) => {
                ClientType::ProtoUser(Arc::clone(proto_user_ptr))
            }
        }
    }
}

type MsgRecvr = mpsc::Receiver<String>;

pub async fn run_write_task(sock: OwnedWriteHalf, mut rx: MsgRecvr) -> Result<(), ioError> {
    /* apparently we can't have ? after await on any of these
     * functions, because await returns (), but recv() and
     * write_all()/flush() shouldn't return (), should they? */
    let mut stream = BufWriter::new(sock);
    while let Some(msg) = rx.recv().await {
        stream.write(msg.as_bytes()).await?;
        stream.flush().await?;
    }
    Ok(())
}

pub async fn run_client_handler(
    id: u64,
    host: Host,
    irc: Arc<Core>,
    tx: MsgSendr,
    sock: OwnedReadHalf,
) {
    let mut handler = ClientHandler::new(id, host, &irc, tx, sock);
    irc.insert_client(handler.id, Arc::downgrade(&handler.client));
    debug!("assigned client id {}", handler.id);

    /* would it be ridic to spawn a new process for every
     * message received from the user, and if we did that
     * what would we do about joining the tasks to check
     * if any of them failed, i.e. require us to shutdown
     * this client and clean up? */
    /* as it stands, process().await means we wait til
     * the fn returns, and inside process() each input
     * line from the client is handled one by one, which
     * is probably fine, who's gonna send additional commands
     * to the server and care whether we process them
     * asynchronously or not? */
    let res = process_lines(&mut handler, &irc).await;

    /* the main listener loop doesn't .await for the return
     * of this function, so it doesn't make sense to have any
     * return value, instead some diagnostics should be printed
     * here if there is any error */
    let death_reason = if let Err(err) = res {
        debug!("Client {} exited with error {}", handler.id, err);
        format!("{}", err)
    } else {
        "Unexpected EOF".to_string()
    };
    /* All the cleanup stuff should just happen on Drop, so I've commented
     * a bunch out for now */

    /* whether we had an error or a graceful return,
     * we need to do some cleanup, namely: remove the client
     * from the hash table the IRC daemon holds of users/
     * clients */
    /*if let ClientType::User(user) = handler.client.get_client_type() {
        let nick = user.get_nick();

        /* clear them from any leftover channels */
        let witnesses = user.clear_chans_and_exit();
    }*/
/*
        match irc.remove_name(&nick) {
            Ok(_name_entity) =>
                debug!("Exit Client {} - freed user with nick: {}",
                        handler.id, &nick),
            Err(err) =>
                warn!("Exit Client {} - free nick {} failed: {}",
                        handler.id, &nick, err),
        }

        /* instead of all this mad stuff it would also be
         * an option to push to id_list vector and then .sort() and .dedup()
         */
        let mut id_list: Vec<u64> = Vec::new();
        {
            let mut user_list: BTreeMap<u64, Arc<User>> = BTreeMap::new();
            for chan in witnesses.iter() {
                let users = chan.gen_user_ptr_vec().clone();
                for user in users.iter() {
                    let id = user.get_id();
                    user_list.insert(id, Arc::clone(&user));
                }
            }

            for key in user_list.keys() {
                id_list.push(*key);
            }
        }

        let line = format!(":{} QUIT :{}", user.get_prefix(), death_reason);
        for id in id_list.iter() {
            if *id == handler.id {
                continue
            }
            if let Some(client_weakptr) = irc.get_client(id) {
                if let Some(client) = Weak::upgrade(&client_weakptr) {
                    if let Err(err) = client.send_line(&line).await {
                        debug!("failed to send to client {}: {}", id, err);
                    }
                }
            }
        }
    }

    /* remove self from main irc Client HashMap */
    if irc.remove_client(&handler.id).is_some() {
        debug!("successfully removed client {} from IRC core hashmap", id);
    } else {
        warn!("attempted removal of our own client {} failed", id);
    }*/
}

/* Receive and process IRC messages */
async fn process_lines(handler: &mut ClientHandler, irc: &Arc<Core>) -> Result<(), GenError> {
    while let Some(line) = handler.stream.next_line().await? {
        if line.is_empty() { continue }
        match error_wrapper(&handler.client, irc, &line).await {
            Err(GenError::IRC(err)) => handler.client.send_err(err).await?,
            Err(GenError::Parse(err)) => handler.client.send_err(ircError::from(err)).await?,
            Err(GenError::Io(err)) => return Err(GenError::Io(err)),
            Err(GenError::Mpsc(err)) => return Err(GenError::Mpsc(err)),
            Err(GenError::DeadClient(user, chans)) => attempt_cleanup(irc, user, chans),
            Err(GenError::DeadUser(nick)) => User::cleanup(irc, &nick),
            Ok(ircReply::None) => (),
            Ok(rpl) => { handler.client.get_user().send_rpl(rpl).await?; }
        }
    }
    Ok(())
}

/* wrapping these two fn calls in this function allows easy error composition,
 * and let's the caller process_lines() catch any errors, relaying parser or
 * IRC errors back to the client, or dropping the client on I/O error */
async fn error_wrapper (client: &Arc<Client>, irc: &Arc<Core>, line: &str) -> Result<ircReply, GenError> {
    let parsed = parse_message(line)?;
    irc::command(irc, client, parsed).await
}

/* found a stale user with no client */
pub fn attempt_cleanup(irc: &Core, user: Arc<User>, chans: Vec<Arc<Channel>>) {
    let id = user.get_id();
    debug!("attempted cleanup of stale User, id {}", id);

    /* irc Core client Hash */
    if let Some(client_weak) = irc.remove_client(&id) {
        debug!("have removed client weak ptr from IRC Clients HashMap");
        if let Some(client) = Weak::upgrade(&client_weak) {
            warn!("unexpectedly, the client pointer does upgrade, in this case remove Userptr from ClientType");
            client.set_client_type(ClientType::Dead);
        } else {
            debug!("it appears to be a dead pointer, as expected")
        }
    } else {
        debug!("client has already been removed from Client hash");
    }
        
    /* irc Core namespace HashMap */
    let nick = user.get_nick();
    if let Ok(NamedEntity::User(_user_weak)) = irc.remove_name(&nick) {
        debug!("remove user ptr of {} from IRC namespace hashmap", nick);
    } else {
        debug!("user ptr for {} has already been removed from IRC namespace/hash table", nick);
    }

    /* search for remaining references in channel lists */
    let found = irc.search_user_chans_purge(&nick);
    debug!("removed user {} from these channels: {}", nick, found.join(" "));

    /* also make sure the user's channel hashmap is also clear */
    user.clear_channel_list();

    /*for chan in chans.iter() {
     *   chan.notify_quit(&user, "vanishes in a cloud of rusty iron shavings").await;
    }*/
}

#[derive(Debug)]
pub struct ClientHandler {
    stream: Lines<BufReader<OwnedReadHalf>>,
    client: Arc<Client>,
    id: u64,
}

impl ClientHandler {
    pub fn new(id: u64, host: Host, irc: &Arc<Core>, tx: MsgSendr, sock: OwnedReadHalf) -> Self {
        ClientHandler {
            stream: BufReader::new(sock).lines(),
            client: Client::new(id, host, irc, tx),
            id,
        }
    }
}

type MsgSendr = mpsc::Sender<String>;

#[derive(Debug)]
pub struct Client {
    client_type: Mutex<ClientType>,
    id: u64,
    host: Host,
    irc: Arc<Core>,
    tx: MsgSendr,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            client_type: Mutex::new(self.client_type.lock().unwrap().clone()),
            id: self.id,
            host: self.host.clone(),
            irc: Arc::clone(&self.irc),
            tx: self.tx.clone(),
        }
    }
}

impl Drop for Client {
    fn drop (&mut self) {
        *self.client_type.lock().unwrap() = ClientType::Dead;
        self.irc.remove_client(&self.id);
    }
}

impl Client {
    pub fn new(id: u64, host: Host, irc: &Arc<Core>, tx: MsgSendr) -> Arc<Self> {
        Arc::new(Client {
            client_type: Mutex::new(ClientType::Unregistered),
            id,
            host,
            irc: Arc::clone(irc),
            tx,
        })
    }

    // don't call this unless is_registered returns true
    pub fn get_user(&self) -> Arc<User> {
        match self.get_client_type() {
            ClientType::User(u_ptr) => Arc::clone(&u_ptr),
            _ => panic!("impossible"),
        }
    }

    pub fn get_host(&self) -> &Host {
        &self.host
    }

    pub fn is_registered(&self) -> bool {
        match self.get_client_type() {
            ClientType::Dead => false,
            ClientType::User(_p) => true,
            ClientType::ProtoUser(_p) => false,
            ClientType::Unregistered => false,
        }
    }

    pub fn get_host_string(&self) -> String {
        match &self.host {
            Host::Hostname(name) => name.to_string(),
            Host::HostAddr(ip_addr) => ip_addr.to_string(),
        }
    }

    pub fn get_client_type(&self) -> ClientType {
        self.client_type.lock().unwrap().clone()
    }

    pub fn set_client_type(&self, new_client_type: ClientType) {
        let mut lock_ptr = self.client_type.lock().unwrap();
        *lock_ptr = new_client_type;
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_irc(&self) -> &Arc<Core> {
        &self.irc
    }

    pub async fn send_err(&self, err: ircError) -> Result<(), GenError> {
        let line = format!(":{} {}", self.irc.get_host(), err);
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        self.send_line(&line).await?;
        Ok(())
    }

    pub async fn send_line(&self, line: &str) -> Result<(), mpscSendErr<String>> {
        let mut string = String::from(line);
        string.push_str("\r\n");
        /* thankfully mpsc::Sender has its own .clone()
         * method, so we don't have to worry about our own
         * Arc/Mutex wrapping, or the problems of holding
         * a mutex across an await */
        self.tx.clone().send(string).await
    }
}
