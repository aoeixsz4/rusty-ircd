/* rusty-ircd - an IRC daemon written in Rust
*  Copyright (C) 2020 Joanna Janet Zaitseva-Doyle <jjadoyle@gmail.com>

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
extern crate log;
extern crate tokio;
extern crate tokio_native_tls;
use crate::io::{ReadHalfWrap, WriteHalfWrap};
use crate::irc::{self, Core, User, NamedEntity};
use crate::irc::message::Message;
use std::error;
use std::fmt;
use std::io::Error as ioError;
use std::net::IpAddr;
use std::sync::{Arc, Weak, Mutex};
use log::{debug, warn};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError as mpscSendErr;
use tokio::task::JoinError as tokJoinErr;
use tokio_native_tls::native_tls::Error as tntTlsErr;

#[derive(Debug)]
pub enum GenError {
    Io(ioError),
    Mpsc(mpscSendErr<String>),
    DeadClient(Arc<User>),
    DeadUser(String),
    TLS(tntTlsErr),
    Tokio(tokJoinErr)
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GenError::Io(ref err) => write!(f, "IO Error: {}", err),
            GenError::Mpsc(ref err) => write!(f, "MPSC Send Error: {}", err),
            GenError::DeadClient(user) => write!(f, "user {}, stale client", user.get_nick()),
            GenError::DeadUser(nick) => write!(f, "user {}, remant, scattered WeakRefs", nick),
            GenError::TLS(ref err) => write!(f, "TLS Error: {}", err),
            GenError::Tokio(ref err) => write!(f, "TLS Error: {}", err)
        }
    }
}

impl error::Error for GenError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            GenError::Io(ref err) => Some(err),
            GenError::Mpsc(ref err) => Some(err),
            GenError::DeadClient(_user) => None,
            GenError::DeadUser(_nick) => None,
            GenError::TLS(ref err) => Some(err),
            GenError::Tokio(ref err) => Some(err)
        }
    }
}

impl From<ioError> for GenError {
    fn from(err: ioError) -> GenError {
        GenError::Io(err)
    }
}

impl From<mpscSendErr<String>> for GenError {
    fn from(err: mpscSendErr<String>) -> GenError {
        GenError::Mpsc(err)
    }
}

impl From<tntTlsErr> for GenError {
    fn from(err: tntTlsErr) -> GenError {
        GenError::TLS(err)
    }
}

impl From<tokJoinErr> for GenError {
    fn from(err: tokJoinErr) -> GenError {
        GenError::Tokio(err)
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

#[derive(Debug)]
pub enum ClientType {
    Dead,
    Unregistered,
    User(Arc<irc::User>),
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

pub async fn run_write_task(sock: WriteHalfWrap, mut rx: MsgRecvr) -> Result<(), ioError> {
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
    sock: ReadHalfWrap,
) {
    let mut handler = ClientHandler::new(id, host, &irc, tx, sock);
    irc.insert_client(handler.id, Arc::downgrade(&handler.client));
    debug!("assigned client id {}", handler.id);
    let res = process_lines(&mut handler, &irc).await;

    if let Err(err) = res {
        debug!("Client {} exited with error {}", handler.id, err);
    } else {
        debug!("{}", "Unexpected EOF".to_string());
    }
}

async fn process_lines(handler: &mut ClientHandler, irc: &Arc<Core>) -> Result<(), GenError> {
    while let Some(line) = handler.stream.next_line().await? {
        if line.is_empty() {
            loop {
                let msg_opt = {
                    let mut lock_ptr = handler.client.msgq.lock().unwrap();
                    if lock_ptr.len() > 0 {
                        Some(lock_ptr.remove(0))
                    } else {
                        None
                    }
                };
                match msg_opt {
                    Some(msg) => handler.client.send(msg).await?,
                    None => break,
                }
            }
        }
        if let Ok(parsed) = line.parse::<Message>() {
            irc::command(irc, &handler.client, parsed).await?;
        }
        loop {
            let msg_opt = {
                let mut lock_ptr = handler.client.msgq.lock().unwrap();
                if lock_ptr.len() > 0 {
                    Some(lock_ptr.remove(0))
                } else {
                    None
                }
            };
            match msg_opt {
                Some(msg) => handler.client.send(msg).await?,
                None => break,
            }
        }
    }
    Ok(())
}

pub fn attempt_cleanup(irc: &Core, user: Arc<User>) {
    let id = user.get_id();
    debug!("attempted cleanup of stale User, id {}", id);

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

    let nick = user.get_nick();
    if let Some(NamedEntity::User(_user_weak)) = irc.remove_name(&nick) {
        debug!("remove user ptr of {} from IRC namespace hashmap", nick);
    } else {
        debug!("user ptr for {} has already been removed from IRC namespace/hash table", nick);
    }
}

#[derive(Debug)]
pub struct ClientHandler {
    stream: Lines<BufReader<ReadHalfWrap>>,
    client: Arc<Client>,
    id: u64,
}

impl ClientHandler {
    pub fn new(id: u64, host: Host, irc: &Arc<Core>, tx: MsgSendr, sock: ReadHalfWrap) -> Self {
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
    msgq: Mutex<Vec<Message>>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client {
            client_type: Mutex::new(self.client_type.lock().unwrap().clone()),
            id: self.id,
            host: self.host.clone(),
            irc: Arc::clone(&self.irc),
            tx: self.tx.clone(),
            msgq: Mutex::new(Vec::new()),
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
            msgq: Mutex::new(Vec::new()),
        })
    }

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

    pub fn queue_msg(&self, msg: Message) {
        let mut lock_ptr = self.msgq.lock().unwrap();
        lock_ptr.push(msg);
    }

    pub async fn send(&self, msg: Message) -> Result<(), GenError> {
        let line = msg.to_string();
        self.send_line(&line).await?;
        Ok(())
    }

    pub async fn send_line(&self, line: &str) -> Result<(), mpscSendErr<String>> {
        let string = String::from(line);
        self.tx.clone().send(string).await
    }
}

pub fn create_host_string(host_var: &Host) -> String {
    match host_var {
        Host::Hostname(hostname_str) => hostname_str.to_string(),
        Host::HostAddr(ip_addr) => ip_addr.to_string(),
    }
}