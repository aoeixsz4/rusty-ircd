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
#[macro_use]
use crate::irc::{self, Core, User};
use crate::irc::error::Error as ircError;
use crate::parser::{parse_message, ParseError};
use std::error;
use std::fmt;
use std::io::Error as ioError;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;

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
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GenError::Io(ref err) => write!(f, "IO Error: {}", err),
            GenError::Parse(ref err) => write!(f, "Parse Error: {}", err),
            GenError::IRC(ref err) => write!(f, "IRC Error: {}", err),
        }
    }
}

impl error::Error for GenError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            // N.B. Both of these implicitly cast `err` from their concrete
            // types (either `&io::Error` or `&num::ParseIntError`)
            // to a trait object `&Error`. This works because both error types
            // implement `Error`.
            GenError::Io(ref err) => Some(err),
            GenError::Parse(ref err) => Some(err),
            GenError::IRC(ref err) => Some(err),
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

impl From<ircError> for GenError {
    fn from(err: ircError) -> GenError {
        GenError::IRC(err)
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
    Unregistered,
    User(Arc<irc::User>),
    //Server(Arc<Mutex<irc::Server>>), leave serv implmentation for much later
    ProtoUser(Arc<Mutex<irc::ProtoUser>>),
}

impl Clone for ClientType {
    fn clone(&self) -> Self {
        match self {
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
) -> Result<(), ioError> {
    let mut handler = ClientHandler::new(id, host, &irc, tx, sock);
    irc.insert_client(handler.id, Arc::downgrade(&handler.client));
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

    /* whether we had an error or a graceful return,
     * we need to do some cleanup, namely: remove the client
     * from the hash table the IRC daemon holds of users/
     * clients */
    match handler.client.get_client_type() {
        ClientType::User(user_ptr) => {
            irc.remove_name(&user_ptr.get_nick());
        }
        _ => (), // do nothing
    }

    /* should probably have a look at res and do something
     * with the errors in there, if there are any... */
    Ok(())
}

/* Receive and process IRC messages */
async fn process_lines(handler: &mut ClientHandler, irc: &Core) -> Result<(), GenError> {
    while let Some(line) = handler.stream.next_line().await? {
        /* an error here is something for the remote user,
         * so whatever the result, we get it in reply and
         * we send that to them - will also need to format as
         * an IRC message, and since there could be more than
         * one message as a reply, we may want to use iterators/
         * combinators to deal with this */
        let reply = match parse_message(&line) {
            Ok(parsed_msg) => {
                /* currently this code will early return on any IRC error,
                 * which is definitely not what we want... need some extra
                 * error composition so irc::command() only returns an actual
                 * error if it's a QUIT/KILL situation */
                irc::command(&irc, &handler.client, parsed_msg).await?
            }
            Err(_) => (), /* TODO: add code to convert parse error to IRC message */
        };
    }
    Ok(())
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
            ClientType::User(u_ptr) => return Arc::clone(&u_ptr),
            _ => panic!("impossible"),
        }
    }

    pub fn get_host(&self) -> &Host {
        &self.host
    }

    pub fn is_registered(&self) -> bool {
        match self.get_client_type() {
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

    pub async fn send_line(&self, line: &str) {
        let mut string = String::from(line);
        string.push_str("\r\n");
        /* thankfully mpsc::Sender has its own .clone()
         * method, so we don't have to worry about our own
         * Arc/Mutex wrapping, or the problems of holding
         * a mutex across an await */
        self.tx.clone().send(string).await;
    }
}
