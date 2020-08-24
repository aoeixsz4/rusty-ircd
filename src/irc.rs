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
pub mod error;
pub mod rfc_defs;
use crate::client;
use crate::client::{Client, ClientType, Host};
use crate::irc::error::Error as ircError;
use crate::parser::ParsedMsg;
use std::clone::Clone;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug)]
pub enum NamedEntity {
    User(Weak<User>), //Chan(Weak<Channel>)
}

impl Clone for NamedEntity {
    fn clone(&self) -> Self {
        match self {
            NamedEntity::User(ptr) => NamedEntity::User(Weak::clone(&ptr)),
        }
    }
}

#[derive(Debug)]
pub struct UserFlags {
    registered: bool,
}

#[derive(Debug)]
pub struct User {
    id: u64,
    nick: Mutex<String>,
    username: String,
    real_name: Mutex<String>,
    host: Host,
    /*channel_list: Vec<Weak<Channel>>,*/
    flags: Mutex<UserFlags>,
    irc: Arc<Core>,
    client: Weak<Client>,
}

impl User {
    pub fn new(
        id: u64,
        irc: &Arc<Core>,
        nick: String,
        username: String,
        real_name: String,
        host: client::Host,
        client: &Arc<Client>,
    ) -> Arc<Self> {
        Arc::new(User {
            id,
            irc: Arc::clone(&irc),
            nick: Mutex::new(nick),
            username,
            real_name: Mutex::new(real_name),
            host,
            client: Arc::downgrade(client),
            flags: Mutex::new(UserFlags { registered: true }), /*channel_list: Mutex::new(Vec::new())*/
        })
    }

    pub fn set_nick(self: Arc<Self>, name: &str) -> Result<(), ircError> {
        /* ? propagates the potential nick in use error */
        self.irc
            .insert_name(name, NamedEntity::User(Arc::downgrade(&self)));
        *self.nick.lock().unwrap() = name.to_string();
        Ok(())
    }

    pub fn get_nick(&self) -> String {
        self.nick.lock().unwrap().clone()
    }

    pub fn get_username(&self) -> String {
        self.username.clone()
    }

    pub fn get_host(&self) -> Host {
        match &self.host {
            Host::Hostname(name) => Host::Hostname(name.clone()),
            Host::HostAddr(ip_addr) => Host::HostAddr(*ip_addr),
        }
    }

    pub fn get_host_string(&self) -> String {
        match &self.host {
            Host::Hostname(name) => name.to_string(),
            Host::HostAddr(ip_addr) => ip_addr.to_string(),
        }
    }

    pub fn get_realname(&self) -> String {
        self.real_name.lock().unwrap().clone()
    }

    pub fn get_prefix(&self) -> String {
        format!("{}!{}@{}", self.get_nick(), self.username, self.get_host_string())
    }

    pub async fn send_msg(&self, src: &User, msg: &str, msg_type: &MsgType) {
        let prefix = src.get_prefix();
        let msg_type_str = match *msg_type {
            MsgType::PrivMsg => "PRIVMSG",
            MsgType::Notice => "NOTICE"
        };
        let line = format!(":{} {} :{}", &prefix, msg_type_str, msg);
        let my_client = self.client.upgrade().unwrap();
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(&line).await;
    }
}

#[derive(Debug)]
pub struct ProtoUser {
    nick: Option<String>,
    username: Option<String>,
    real_name: Option<String>,
}

#[derive(Debug)]
pub struct Core {
    namespace: Mutex<HashMap<String, NamedEntity>>,
    clients: Mutex<HashMap<u64, Weak<Client>>>,
    users: Mutex<HashMap<String, Weak<User>>>,
    //channels: Mutex<HashMap<String, Weak<Channel>>>,
    id_counter: Mutex<u64>, //servers: Mutex<HashMap<u64, Arc<Server>>>,
}

impl Core {
    // init hash tables
    pub fn new() -> Arc<Self> {
        let clients = Mutex::new(HashMap::new());
        //let servers  = Mutex::new(HashMap::new());
        let users = Mutex::new(HashMap::new());
        //let channels = Mutex::new(HashMap::new());
        let namespace = Mutex::new(HashMap::new());
        let id_counter = Mutex::new(0);
        Arc::new(Core {
            clients,
            //channels,
            users,
            namespace,
            id_counter, //servers
        })
    }

    pub fn assign_id(&self) -> u64 {
        let mut lock_ptr = self.id_counter.lock().unwrap();
        *lock_ptr += 1;
        *lock_ptr
    }

    pub fn insert_client(&self, id: u64, client: Weak<Client>) {
        self.clients.lock().unwrap().insert(id, client);
    }

    pub fn insert_name(&self, name: &str, item: NamedEntity) -> Result<(), ircError> {
        let mut hashmap = self.namespace.lock().unwrap();
        if !hashmap.contains_key(name) {
            hashmap.insert(name.to_string(), item);
            Ok(())
        } else {
            Err(self::error::ERR_NICKNAMEINUSE)
        }
    }

    pub fn remove_name(&self, name: &str) -> Result<NamedEntity, ircError> {
        let mut hashmap = self.namespace.lock().unwrap();
        hashmap
            .remove(name)
            .ok_or_else(|| self::error::ERR_NOSUCHNICK)
    }

    pub fn get_client(&self, id: &u64) -> Option<Weak<Client>> {
        self.clients
            .lock()
            .unwrap()
            .get(id)
            .map(|cli| Weak::clone(cli))
    }

    pub fn remove_client(&self, id: &u64) -> Option<Weak<Client>> {
        self.clients.lock().unwrap().remove(id)
    }

    pub fn get_name(&self, name: &str) -> Result<NamedEntity, ircError> {
        match self.namespace.lock().unwrap().get(name) {
            Some(NamedEntity::User(user_ptr)) => Ok(NamedEntity::User(Weak::clone(&user_ptr))),
            //Some(NamedEntity::Chan(chan_ptr)) =>
            //  Some(NamedEntity::Chan(Weak::clone(&chan_ptr))),
            None => Err(self::error::ERR_NOSUCHNICK),
        }
    }

    pub fn register(
        &self,
        client: &Arc<Client>,
        nick: String,
        username: String,
        real_name: String,
    ) -> Arc<User> {
        let host_str = client.get_host_string();
        let host = client.get_host();
        let id = client.get_id();
        let irc = client.get_irc();
        println!(
            "register user {}!{}@{}, Real name: {}",
            &nick, &username, &host_str, &real_name
        );
        let user = User::new(
            id,
            irc,
            nick.to_string(),
            username,
            real_name.clone(),
            host.clone(),
            client,
        );
        self.insert_name(&nick, NamedEntity::User(Arc::downgrade(&user)));
        user
    }
}

#[derive(Debug)]
pub enum MsgType {
    PrivMsg,
    Notice
}

pub async fn command(
    irc: &Core,
    client: &Arc<Client>,
    params: ParsedMsg,
) -> Result<(), ircError> {
    let registered = client.is_registered();

    match &params.command[..] {
        "NICK" => nick(irc, client, params),
        "USER" => user(irc, client, params),
        "PRIVMSG" if registered => Ok(msg(irc, &client.get_user(), params, MsgType::PrivMsg).await?),
        "NOTICE" if registered =>
        /* RFC states NOTICE messages don't get replies */
        {
            msg(&irc, &client.get_user(), params, MsgType::Notice).await;
            Ok(())
        },
        "PRIVMSG" | "NOTICE" if ! registered => Err(self::error::ERR_NOTREGISTERED),
        _ => Err(self::error::ERR_UNKNOWNCOMMAND),
    }
}

pub async fn msg(irc: &Core, user: &User, mut params: ParsedMsg, msg_type: MsgType) -> Result<(), ircError> {
    if params.opt_params.len() < 1 {
        return Err(self::error::ERR_NORECIPIENT);
    }
    let targets = params.opt_params.remove(0);

    // if there are more than two arguments,
    // concatenate the remainder to one string
    let message = params.opt_params.join(" ");
    // if there were no more args, message should be an empty String
    if message.len() == 0 {
        return Err(self::error::ERR_NOTEXTTOSEND);
    }
    println!("target is {} and content is {}", targets, message);

    // loop over targets
    for target in targets.split(',') {
        if let NamedEntity::User(user_weak) = irc.get_name(target)? {
            Weak::upgrade(&user_weak)
                .unwrap()
                .send_msg(&user, &message, &msg_type)
                .await;
        }
    }
    Ok(())
}

pub fn user(irc: &Core, client: &Arc<Client>, params: ParsedMsg) -> Result<(), ircError> {
    // a USER command should have exactly four parameters
    // <username> <hostname> <servername> <realname>,
    // though we ignore the middle two unless a server is
    // forwarding the message
    let args = params.opt_params;
    if args.len() != 4 {
        // strictly speaking this should be an RFC-compliant
        // numeric error ERR_NEEDMOREPARAMS
        return Err(self::error::ERR_NEEDMOREPARAMS);
    }
    let username = args[0].clone();
    let real_name = args[3].clone();

    // tuple Some(&str), Some(ClientType), bool died
    let result = match client.get_client_type() {
        ClientType::Unregistered => {
            // initiate handshake
            Some(ClientType::ProtoUser(Arc::new(Mutex::new(ProtoUser {
                nick: None,
                username: Some(username),
                real_name: Some(real_name),
            }))))
        }
        ClientType::User(_user_ref) => {
            // already registered! can't change username
            return Err(self::error::ERR_ALREADYREGISTRED);
        }
        ClientType::ProtoUser(proto_user_ref) => {
            // got nick already? if so, complete registration
            let proto_user = proto_user_ref.lock().unwrap();
            if let Some(nick) = &proto_user.nick {
                // had nick already, complete registration
                Some(ClientType::User(irc.register(
                    client,
                    nick.clone(),
                    username,
                    real_name,
                )))
            // there probably is some message we're meant to
            // return to the client to confirm successful
            // registration...
            } else {
                // don't see an error in the irc file,
                // except the one if you're already reg'd
                // NOTICE_BLOCKY
                proto_user_ref.lock().unwrap().username = Some(username);
                proto_user_ref.lock().unwrap().real_name = Some(real_name);
                None
            }
        } //ClientType::Server(_server_ref) => (None, None, false)
    };

    if let Some(new_client_type) = result {
        client.set_client_type(new_client_type);
    }
    return Ok(());
}

pub fn nick(irc: &Core, client: &Arc<Client>, params: ParsedMsg) -> Result<(), ircError> {
    let nick;
    if let Some(n) = params.opt_params.iter().next() {
        nick = n.to_string();
    } else {
        return Err(self::error::ERR_NEEDMOREPARAMS);
    }

    // is this nick already taken?
    if let Ok(_hit) = irc.get_name(&nick) {
        return Err(self::error::ERR_NICKNAMEINUSE);
    }

    // we can return a tuple and send messages after the match
    // to avoid borrowing mutably inside the immutable borrow
    // (Some(&str), Some(ClientType), bool died)
    let result = match client.get_client_type() {
        ClientType::Unregistered => {
            // in this case we need to create a "proto user"
            Some(ClientType::ProtoUser(Arc::new(Mutex::new(ProtoUser {
                nick: Some(nick),
                username: None,
                real_name: None,
            }))))
        }
        ClientType::User(user_ref) => {
            // just a nick change
            user_ref.set_nick(&nick)?;
            None
        }
        ClientType::ProtoUser(proto_user_ref) => {
            // in this case we already got USER
            let mut proto_user = proto_user_ref.lock().unwrap();
            // need to account for the case where NICK is sent
            // twice without any user command
            if let Some(_) = proto_user.nick {
                proto_user.nick = Some(nick);
                None
            } else {
                // full registration! wooo
                let username = proto_user.username.as_ref();
                let real_name = proto_user.real_name.as_ref();
                Some(ClientType::User(irc.register(
                    client,
                    nick,
                    username.unwrap().to_string(),
                    real_name.unwrap().to_string(),
                )))
            }
        }
    };

    if let Some(new_client_type) = result {
        client.set_client_type(new_client_type);
    }
    return Ok(());
}
