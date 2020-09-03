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
pub mod chan;
pub mod error;
pub mod reply;
pub mod rfc_defs;
use crate::client;
use crate::client::{Client, ClientType, GenError, Host};
use crate::irc::chan::{ChanFlags, Channel};
use crate::irc::error::Error as ircError;
use crate::irc::reply::Reply as ircReply;
use crate::irc::rfc_defs as rfc;
use crate::parser::ParsedMsg;
use std::clone::Clone;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

#[derive(Debug)]
pub enum NamedEntity {
    User(Weak<User>),
    Chan(Arc<Channel>),
}

impl Clone for NamedEntity {
    fn clone(&self) -> Self {
        match self {
            NamedEntity::User(ptr) => NamedEntity::User(Weak::clone(&ptr)),
            NamedEntity::Chan(ptr) => NamedEntity::Chan(Arc::clone(&ptr)),
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

    pub fn set_nick(self: &Arc<Self>, name: &str) -> Result<(), ircError> {
        let old_nick = self.nick.lock().unwrap().to_string();
        /* ? propagates the potential nick in use error */
        self.irc
            .insert_name(name, NamedEntity::User(Arc::downgrade(&self)))?;
        *self.nick.lock().unwrap() = name.to_string();
        self.irc.remove_name(&old_nick)?;
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
        format!(
            "{}!{}@{}",
            self.get_nick(),
            self.username,
            self.get_host_string()
        )
    }

    pub async fn send_msg(
        &self,
        src: &User,
        target: &str,
        msg: &str,
        msg_type: &MsgType,
    ) -> Result<(), GenError> {
        let prefix = src.get_prefix();
        let command_str = match *msg_type {
            MsgType::PrivMsg => "PRIVMSG",
            MsgType::Notice => "NOTICE",
        };
        let line = format!(":{} {} {} :{}", &prefix, command_str, target, msg);
        let my_client = self.client.upgrade().unwrap();
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(&line).await?;
        Ok(())
    }

    pub async fn send_err(&self, err: ircError) -> Result<(), GenError> {
        let line = format!("{}", err);
        let my_client = self.client.upgrade().unwrap();
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(&line).await?;
        Ok(())
    }

    pub async fn send_rpl(&self, reply: ircReply) -> Result<(), GenError> {
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        let line = format!("{}", reply);
        if line.len() > rfc::MAX_MSG_SIZE - 2 {
            match reply {
                /* not all can be recursed */
                ircReply::NameReply(chan, mut nick_vec) => {
                    /* "353 {} :{}<CR><LF>" */
                    let overhead = rfc::MAX_MSG_PARAMS - (8 + chan.len());
                    let mut vec_len = nick_vec.len();
                    let mut i = 0;
                    let mut sum = 0;

                    /* count how many strings we can fit */
                    while i < vec_len {
                        if sum + nick_vec[i].len() >= overhead {
                            let temp = nick_vec.split_off(i);
                            let line = format!("{}", ircReply::NameReply(chan.clone(), nick_vec));
                            let my_client = self.client.upgrade().unwrap();
                            my_client.send_line(&line).await?;
                            nick_vec = temp;
                            i = 0;
                            sum = 0;
                            vec_len = nick_vec.len();
                        }
                    }

                    Ok(())
                }
                _ => Ok(()),
            }
        } else {
            let my_client = self.client.upgrade().unwrap();
            my_client.send_line(&line).await?;
            Ok(())
        }
    }

    pub async fn send_line(&self, line: &str) -> Result<(), GenError> {
        let my_client = self.client.upgrade().unwrap();
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(line).await?;
        Ok(())
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
            Err(ircError::NicknameInUse(name.to_string()))
        }
    }

    pub fn remove_name(&self, name: &str) -> Result<NamedEntity, ircError> {
        let mut hashmap = self.namespace.lock().unwrap();
        hashmap
            .remove(name)
            .ok_or_else(|| ircError::NoSuchNick(name.to_string()))
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

    pub fn get_name(&self, name: &str) -> Option<NamedEntity> {
        self.namespace.lock().unwrap().get(name).cloned()
    }

    pub fn get_nick(&self, nick: &str) -> Result<NamedEntity, ircError> {
        if let Some(named_entity) = self.get_name(nick) {
            Ok(named_entity)
        } else {
            Err(ircError::NoSuchNick(nick.to_string()))
        }
    }

    pub fn get_chan(&self, chanmask: &str) -> Option<Arc<Channel>> {
        if let Some(NamedEntity::Chan(chan)) = self.get_name(chanmask) {
            Some(chan)
        } else {
            None
        }
    }

    pub async fn part_chan(
        &self,
        chanmask: &str,
        user: &Arc<User>,
        part_msg: &str,
    ) -> Result<(), GenError> {
        if let Some(chan) = self.get_chan(chanmask) {
            if !chan.is_joined(user) {
                user.send_err(ircError::NotOnChannel(chanmask.to_string()))
                    .await
            } else {
                if let Some(key) = chan.get_user_key(user) {
                    chan.rm_key(&key);
                }
                if chan.is_empty() {
                    self.remove_name(chanmask)?;
                    Ok(())
                } else {
                    chan.notify_part(user, chanmask, part_msg).await
                }
            }
        } else {
            user.send_err(ircError::NoSuchChannel(chanmask.to_string()))
                .await
        }
    }

    pub async fn join_chan(&self, chanmask: &str, user: &Arc<User>) -> Result<(), GenError> {
        if !rfc::valid_channel(chanmask) {
            return user
                .send_err(ircError::NoSuchChannel(chanmask.to_string()))
                .await;
        }
        let channel = self.get_chan(chanmask);
        let chan = if let Some(chan) = channel {
            /* need to check if user is already in chan */
            if chan.is_joined(user) {
                return Ok(());
            }
            chan.add_user(user, ChanFlags::None);
            if let Err(err) = chan.notify_join(user, chanmask).await {
                println!("encountered error: {}", err);
            }
            chan
        } else {
            let chan = Arc::new(Channel::new(chanmask));
            self.insert_name(chanmask, NamedEntity::Chan(Arc::clone(&chan)))?; // what happens if this error does occur?
            chan.add_user(user, ChanFlags::Op);
            chan
        };
        user.send_rpl(ircReply::Topic(chanmask.to_string(), chan.get_topic()))
            .await?;

        user.send_rpl(ircReply::NameReply(
            chanmask.to_string(),
            chan.gen_sorted_nick_list(),
        ))
        .await?;

        user.send_rpl(ircReply::EndofNames(chanmask.to_string()))
            .await
    }

    pub fn register(
        &self,
        client: &Arc<Client>,
        nick: String,
        username: String,
        real_name: String,
    ) -> Result<Arc<User>, ircError> {
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
            real_name,
            host.clone(),
            client,
        );
        self.insert_name(&nick, NamedEntity::User(Arc::downgrade(&user)))?;
        Ok(user)
    }
}

#[derive(Debug)]
pub enum MsgType {
    PrivMsg,
    Notice,
}

pub async fn command(irc: &Core, client: &Arc<Client>, params: ParsedMsg) -> Result<(), GenError> {
    let registered = client.is_registered();
    let cmd = params.command.to_ascii_uppercase();

    match &cmd[..] {
        "NICK" => nick(irc, client, params).await,
        "USER" => user(irc, client, params).await,
        "PRIVMSG" if registered => msg(irc, &client.get_user(), params, MsgType::PrivMsg).await,
        "NOTICE" if registered => msg(&irc, &client.get_user(), params, MsgType::Notice).await,
        "JOIN" if registered => join(&irc, &client.get_user(), params).await,
        "PART" if registered => part(&irc, &client.get_user(), params).await,
        "PART" | "JOIN" | "PRIVMSG" | "NOTICE" if !registered => {
            client.send_err(ircError::NotRegistered).await
        }
        _ => {
            client
                .send_err(ircError::UnknownCommand(params.command.to_string()))
                .await
        }
    }
}

pub async fn join(irc: &Core, user: &Arc<User>, mut params: ParsedMsg) -> Result<(), GenError> {
    if params.opt_params.is_empty() {
        return user
            .send_err(ircError::NeedMoreParams("JOIN".to_string()))
            .await;
    }

    /* JOIN can take a second argument. The format is:
     * JOIN comma,sep.,chan,list comma,sep.,key,list
     * but I'll leave key implementation til later */
    let targets = params.opt_params.remove(0);
    for target in targets.split(',') {
        let result = irc.join_chan(&target, user).await;
        // error here is probably I/O - dropped client
        if let Err(err) = result {
            println!("another client pooped out: {}", err);
        }
    }
    Ok(())
}

pub async fn part(irc: &Core, user: &Arc<User>, mut params: ParsedMsg) -> Result<(), GenError> {
    if params.opt_params.is_empty() {
        return user
            .send_err(ircError::NeedMoreParams("PART".to_string()))
            .await;
    }

    let targets = params.opt_params.remove(0);
    let part_msg = if params.opt_params.is_empty() {
        String::from("")
    } else {
        params.opt_params.remove(0)
    };
    for target in targets.split(',') {
        let result = irc.part_chan(&target, user, &part_msg).await;
        // error here is probably I/O - dropped client
        if let Err(err) = result {
            println!("another client pooped out: {}", err);
        }
    }
    Ok(())
}
pub async fn msg(
    irc: &Core,
    user: &User,
    mut params: ParsedMsg,
    msg_type: MsgType,
) -> Result<(), GenError> {
    if params.opt_params.is_empty() {
        match msg_type {
            MsgType::Notice => return Ok(()),
            MsgType::PrivMsg => {
                return user
                    .send_err(ircError::NoRecipient("PRIVMSG".to_string()))
                    .await
            }
        }
    }
    let targets = params.opt_params.remove(0);

    // if there are more than two arguments,
    // concatenate the remainder to one string
    let message = params.opt_params.join(" ");
    // if there were no more args, message should be an empty String
    if message.is_empty() {
        return Err(GenError::from(ircError::NoTextToSend));
    }
    println!("target is {} and content is {}", targets, message);

    // loop over targets
    for target in targets.split(',') {
        match irc.get_nick(target)? {
            NamedEntity::User(user_weak) => {
                let result = Weak::upgrade(&user_weak)
                    .unwrap()
                    .send_msg(&user, &target, &message, &msg_type)
                    .await;
                if let Err(err) = result {
                    // write debugging output, but don't do anything otherwise,
                    // let whatever task handles this client drop it
                    println!("send_message to {} failed: {}", &target, err);
                }
            }
            NamedEntity::Chan(chan) => {
                let result = chan.send_msg(&user, &target, &message, &msg_type).await;
                if let Err(err) = result {
                    match msg_type {
                        MsgType::PrivMsg => user.send_err(err).await?,
                        MsgType::Notice => println!(
                            "not sending error {} to user {}",
                            err,
                            user.nick.lock().unwrap().to_string()
                        ),
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn user(irc: &Core, client: &Arc<Client>, params: ParsedMsg) -> Result<(), GenError> {
    // a USER command should have exactly four parameters
    // <username> <hostname> <servername> <realname>,
    // though we ignore the middle two unless a server is
    // forwarding the message
    let args = params.opt_params;
    if args.len() != 4 {
        // strictly speaking this should be an RFC-compliant
        // numeric error ERR_NEEDMOREPARAMS
        return client
            .send_err(ircError::NeedMoreParams("USER".to_string()))
            .await;
    }
    let username = args[0].clone();
    let real_name = args[3].clone();

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
            return client.send_err(ircError::AlreadyRegistred).await;
        }
        ClientType::ProtoUser(proto_user_ref) => {
            // got nick already? if so, complete registration
            let proto_user = proto_user_ref.lock().unwrap();
            if let Some(nick) = &proto_user.nick {
                // had nick already, complete registration
                Some(ClientType::User(
                    irc.register(client, nick.clone(), username, real_name)?, // propagate the error if it goes wrong
                )) // (nick taken, most likely corner-case)
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
    Ok(())
}

pub async fn nick(irc: &Core, client: &Arc<Client>, params: ParsedMsg) -> Result<(), GenError> {
    let nick;
    if let Some(n) = params.opt_params.iter().next() {
        nick = n.to_string();
    } else {
        return client
            .send_err(ircError::NeedMoreParams("NICK".to_string()))
            .await;
    }

    // is the nick a valid nick string?
    if !rfc::valid_nick(&nick) {
        return client
            .send_err(ircError::ErroneusNickname(nick.to_string()))
            .await;
    }

    // is this nick already taken?
    if let Some(_hit) = irc.get_name(&nick) {
        return client
            .send_err(ircError::NicknameInUse(nick.to_string()))
            .await;
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
            if proto_user.nick.is_some() {
                proto_user.nick = Some(nick);
                None
            } else {
                // full registration! wooo
                let username = proto_user.username.as_ref();
                let real_name = proto_user.real_name.as_ref();
                Some(ClientType::User(
                    irc.register(
                        client,
                        nick,
                        username.unwrap().to_string(),
                        real_name.unwrap().to_string(),
                    )?, // error propagation if registration fails
                ))
            }
        }
    };

    if let Some(new_client_type) = result {
        client.set_client_type(new_client_type);
    }
    Ok(())
}
