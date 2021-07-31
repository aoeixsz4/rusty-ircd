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
pub mod chan;
pub mod error;
pub mod reply;
pub mod rfc_defs;
pub mod message;
use crate::{USER_MODES, CHAN_MODES};
use crate::client;
use crate::client::{Client, ClientType, ClientReply, ClientReplies, GenError, Host};
use crate::irc::chan::{ChanFlags, Channel, ChanTopic};
use crate::irc::error::Error as ircError;
use crate::irc::reply::Reply as ircReply;
use crate::irc::rfc_defs as rfc;
use crate::irc::message::Message;
extern crate log;
extern crate chrono;
use chrono::Utc;
use log::{debug, warn, trace};
use std::clone::Clone;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};


macro_rules! gef {
    ($e:expr) => (Err(GenError::from($e)));
}

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

#[derive(Debug, Clone)]
pub struct UserFlags {
    registered: bool
}

#[derive(Debug)]
pub struct User {
    id: u64,
    nick: Mutex<String>,
    username: String,
    real_name: Mutex<String>,
    host: Host,
    server: String,
    channel_list: Mutex<HashMap<String, Weak<Channel>>>,
    flags: Mutex<UserFlags>,
    irc: Arc<Core>,
    client: Weak<Client>,
}

impl Clone for User {
    fn clone(&self) -> Self {
        User {
            id: self.id,
            nick: Mutex::new(self.nick.lock().unwrap().clone()),
            username: self.username.clone(),
            real_name: Mutex::new(self.real_name.lock().unwrap().clone()),
            host: self.host.clone(),
            server: self.server.clone(),
            channel_list: Mutex::new(self.channel_list.lock().unwrap().clone()),
            flags: Mutex::new(self.flags.lock().unwrap().clone()),
            irc: Arc::clone(&self.irc),
            client: Weak::clone(&self.client)
        }
    }
}

impl Drop for User {
    fn drop (&mut self) {
        debug!("drop called on user {}, clear channel list", self.get_nick());
        self.clear_up();
    }
}

impl User {
    pub fn new(
        id: u64,
        irc: &Arc<Core>,
        nick: String,
        username: String,
        real_name: String,
        host: client::Host,
        server: String,
        client: &Arc<Client>,
    ) -> Arc<Self> {
        Arc::new(User {
            id,
            irc: Arc::clone(&irc),
            nick: Mutex::new(nick),
            username,
            real_name: Mutex::new(real_name),
            host,
            server,
            channel_list: Mutex::new(HashMap::new()),
            client: Arc::downgrade(client),
            flags: Mutex::new(UserFlags { registered: true }), /*channel_list: Mutex::new(Vec::new())*/
        })
    }

    /* since this is basically the drop() code,
     * have drop just call this */
    pub fn clear_up(&self) {
        self.channel_list.lock()
            .unwrap()
            .drain()
            .filter_map(|(_name, chan_ptr)|{
                Weak::upgrade(&chan_ptr)
                /* but is it bad to silently ignore the refs that won't upgrade... */
            }).for_each(|chan|{
                chan.rm_key(&self.get_nick());
                if chan.is_empty() {
                    if let Err(err) = self.irc.remove_name(&chan.get_name()) {
                        warn!("error {} removing non-existant channel {}", err, &chan.get_name());
                    }
                }
            });
        if let Err(err) = self.irc.remove_name(&self.get_nick()) {
            warn!("error {} removing non-existant nick {}", err, &self.get_nick());
        }
    }

    /* attempt to find and upgrade a pointer to the user's client,
     * if that fails, so some cleanup and return an error indicating
     * dead client or similar */
    pub fn fetch_client(self: &Arc<Self>) -> Result<Arc<Client>, GenError> { /* GDB++ */
        Weak::upgrade(&self.client).ok_or_else(|| {
            self.clear_up();
            debug!("fetch_client(): got a dead client @ user {}", self.get_nick());
            /* can't iterate here as chan.notify_quit() will call
             * user.send_line() and make this fn recursive */
            GenError::DeadClient(Arc::clone(&self))
        })
    }

    /* nick changes need to be done carefully and atomically, or they'll
     * lead to race conditions and mess with book-keeping (unless I stop
     * relying on purely text based keys for some User/Channel management) */
    pub fn change_nick(self: &Arc<Self>, name: &str) -> Result<ircReply, GenError> {
        self.irc.try_nick_change(self, name)
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_channel_list(&self) -> Vec<Weak<Channel>> {
        let mut values = Vec::new();
        for val in self.channel_list.lock().unwrap().values() {
            values.push(Weak::clone(&val));
        }
        values
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

    pub fn get_server(&self) -> String {
        self.server.clone()
    }

    pub async fn send_msg(
        self: &Arc<Self>,
        src: &User,
        command_str: &str,
        target: &str,
        msg: &str
    ) -> Result<ClientReply, GenError> { /* GDB+ */
        let prefix = src.get_prefix();
        let line = format!(":{} {} {} :{}", &prefix, command_str, target, msg);
        /* instead of unwrap(), fetch_client() tries to upgrade the pointer,
         * if that fails it does some cleaning up and returns a GenError::Io(unexpected Eof)
         */
        let my_client = self.fetch_client()?;
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(&line).await?;
        Ok(Ok(ircReply::None))
    }

    pub async fn send_err(self: &Arc<Self>, err: ircError) -> Result<ircReply, GenError> { /* GDB+ */
        let line = format!(":{} {}", self.irc.get_host(), err);
        let my_client = self.fetch_client()?;
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(&line).await?;
        Ok(ircReply::None)
    }

    pub async fn send_rpl(self: &Arc<Self>, reply: ircReply) -> Result<ircReply, GenError> { /* GDB+ */
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        let mut line = reply.format(&self.get_server(), &self.get_nick());
        let my_client = self.fetch_client()?;
        /* break up long messages if neccessary,
         * reply::split essentially returns line, None when
         * line is not larger than MAX_MSG_SIZE */
        loop {
            let (trim, rest_opt) = reply::split(&line);
            my_client.send_line(&trim).await?;
            if let Some(rest) = rest_opt {
                line = rest;
            } else {
                break;
            }
        }
        Ok(ircReply::None)
    }

    pub async fn send_line(self: &Arc<Self>, line: &str) -> Result<ircReply, GenError> { /* GDB++ */
        let my_client = self.fetch_client()?;
        /* passing to an async fn and awaiting on it is gonna
         * cause lifetime problems with a &str... */
        my_client.send_line(line).await?;
        Ok(ircReply::None)
    }

    pub fn upgrade(weak_ptr: &Weak<Self>, nick: &str) -> Result<Arc<Self>, GenError> { /* GDB+++ */
        if let Some(good_ptr) = Weak::upgrade(&weak_ptr) {
            Ok(good_ptr)
        } else {
            Err(GenError::DeadUser(nick.to_string()))
        }
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
    id_counter: Mutex<u64>, //servers: Mutex<HashMap<u64, Arc<Server>>>,
    hostname: String,
    version: String,
    date: String,
    user_modes: String,
    chan_modes: String
}

impl Core {
    // init hash tables
    pub fn new(hostname: String, version: String) -> Arc<Self> {
        let clients = Mutex::new(HashMap::new());
        //let servers  = Mutex::new(HashMap::new());
        let namespace = Mutex::new(HashMap::new());
        let id_counter = Mutex::new(0);
        Arc::new(Core {
            clients,
            namespace, // combined nick and channel HashMap
            id_counter, //servers
            hostname,
            version,
            date: Utc::now().to_rfc2822(),
            user_modes: String::from(USER_MODES),
            chan_modes: String::from(CHAN_MODES)
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
            debug!("added key {} hashmap, size = {}", name, hashmap.len());
            Ok(())
        } else {
            Err(ircError::NicknameInUse(name.to_string()))
        }
    }

    pub fn remove_name(&self, name: &str) -> Result<NamedEntity, ircError> {
        let mut hashmap = self.namespace.lock().unwrap();
        let ret = hashmap
            .remove(name)
            .ok_or_else(|| ircError::NoSuchNick(name.to_string()));
        if ret.is_ok() {
            debug!("removed key {} from hashmap, size = {}", name, hashmap.len());
        }
        ret
    }

    pub fn get_host(&self) -> String {
        self.hostname.clone()
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

    pub fn get_nick(&self, nick: &str) -> Option<Weak<User>> {
        if let Some(NamedEntity::User(u_ptr)) = self.get_name(nick) {
            Some(u_ptr)
        } else {
            None
        }
    }

    pub fn get_chan(&self, chanmask: &str) -> Result<Arc<Channel>, ircError> {
        if let Some(NamedEntity::Chan(chan)) = self.get_name(chanmask) {
            Ok(chan)
        } else {
            Err(ircError::NoSuchChannel(chanmask.to_string()))
        }
    }

    pub fn get_chanmodes(&self) -> String {
        self.chan_modes.clone()
    }

    pub fn get_date(&self) -> String {
        self.date.clone()
    }

    pub fn list_chans_ptr(&self) -> Vec<Arc<Channel>> {
        let mutex_lock = self.namespace.lock().unwrap();
        let mut ret = Vec::new();
        for ent in mutex_lock.values() {
            if let NamedEntity::Chan(chan) = ent {
                ret.push(Arc::clone(&chan));
            }
        }
        ret
    }

    pub fn list_chans_str(&self) -> Vec<String> {
        let vector = self.list_chans_ptr();
        let mut ret = Vec::new();
        for item in vector {
            ret.push(item.get_name())
        }; ret
    }

    pub fn get_list_reply(&self) -> Vec<(Arc<Channel>, Option<ChanTopic>)> {
        let vector = self.list_chans_ptr();
        let mut out_vect = Vec::new();
        for item in vector {
            out_vect.push((Arc::clone(&item), item.get_topic()));
        } out_vect
    }

    pub fn get_umodes(&self) -> String {
        self.user_modes.clone()
    }

    pub fn get_version(&self) -> String {
        self.version.clone()
    }

    pub async fn part_chan(
        &self,
        chanmask: &str,
        user: &Arc<User>,
        part_msg: &str,
    ) -> Result<ircReply, ircError> {
        let chan = self.get_chan(chanmask)?;
        chan.rm_user(user, part_msg).await.map_err(|_e|{
                ircError::NotOnChannel(chanmask.to_string())
            })?;
        Ok(ircReply::None)
    }

    pub async fn join_chan(self: &Arc<Core>, chanmask: &str, user: &Arc<User>) -> Result<ClientReplies, GenError> {
        let mut replies = Vec::new();
        if !rfc::valid_channel(chanmask) {
            replies.push(Err(ircError::NoSuchChannel(chanmask.to_string())));
            return Ok(replies);
        }
        let nick = user.get_nick();
        match self.get_chan(chanmask) {
            Ok(chan) => {
                /* need to check if user is already in chan */
                if chan.is_joined(&nick) {
                    return Ok(replies);
                }
                chan.add_user(user, ChanFlags::None).await
            },
            Err(_) => {
                let chan = Arc::new(Channel::new(&self, chanmask));
                self.insert_name(chanmask, NamedEntity::Chan(Arc::clone(&chan)))?; // what happens if this error does occur?
                chan.add_user(user, ChanFlags::Op).await
            }
        }
    }

    /* don't want anyone to take our nick while we're in the middle of faffing around... */
    pub fn try_nick_change(&self, user: &User, new_nick: &str) -> Result<ircReply, GenError> {
        let mut big_fat_mutex_lock = self.namespace.lock().unwrap();
        let mut chanlist_mutex_lock = user.channel_list.lock().unwrap();
        let nick = new_nick.to_string();
        let old_nick = user.get_nick();
        if big_fat_mutex_lock.contains_key(&nick) {
            gef!(ircError::NicknameInUse(nick))
        } else {
            if let Some(val) = big_fat_mutex_lock.remove(&old_nick) {
                /* move to new key */
                big_fat_mutex_lock.insert(nick.clone(), val);

                /* update User struct */
                *user.nick.lock().unwrap() = nick;

                /* update channels list */
                for (chan_name, chan_wptr) in chanlist_mutex_lock.clone().iter() {
                    if let Some(chan) = Weak::upgrade(&chan_wptr) {
                        if let Err(err) = chan.update_nick(&old_nick, &new_nick) {
                            warn!("try to update nick {} in chan {} despite not being in chan, error: {}", &chan_name, &old_nick, err);
                        }
                    } else {
                        debug!("try_nick_change(): can't upgrade pointer to {}, deleting key", chan_name);
                        chanlist_mutex_lock.remove(chan_name);
                    }
                }
            }
            Ok(ircReply::None)
        }
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
        let server = irc.hostname.clone();
        trace!(
            "register user {}!{}@{}, Real name: {} -- client id {}",
            &nick, &username, &host_str, &real_name, id
        );
        let user = User::new(
            id,
            irc,
            nick.to_string(),
            username,
            real_name,
            host.clone(),
            server,
            client,
        );
        self.insert_name(&nick, NamedEntity::User(Arc::downgrade(&user)))?;
        Ok(user)
    }

    /* think a bit more about what this method is doing and what it's for */
    fn _search_user_chans(&self, nick: &str, purge: bool) -> Vec<String> {
        let mut channels = Vec::new();
        let mut chan_strings = Vec::new();
        for value in self.namespace.lock().unwrap().values() {
            if let NamedEntity::Chan(chan_ptr) = value {
                channels.push(Arc::clone(&chan_ptr));
            }
        }

        for channel in channels.iter() {
            if channel.is_joined(nick) {
                chan_strings.push(channel.get_name());
                if purge {
                    channel.rm_key(&nick);
                    if channel.is_empty() && self.remove_name(&channel.get_name()).is_ok() {
                        debug!("_search_user_chans(): remove channel {} from IRC HashMap", &channel.get_name());
                    }
                }
            }
        }

        chan_strings
    }

    pub fn search_user_chans(&self, nick: &str) -> Vec<String> {
        self._search_user_chans(nick, false)
    }

    pub fn search_user_chans_purge(&self, nick: &str) -> Vec<String> {
        self._search_user_chans(nick, true)
    }
}

#[derive(Debug)]
pub enum MsgType {
    PrivMsg,
    Notice,
}

pub async fn command(irc: &Arc<Core>, client: &Arc<Client>, message: Message) -> Result<ClientReplies, GenError> {
    let registered = client.is_registered();
    let cmd = message.command.to_ascii_uppercase();
    let params = message.parameters;

    match &cmd[..] {
        "NICK" => nick(irc, client, params).await,
        "USER" => user(irc, client, params).await,
        "PRIVMSG" if registered => msg(irc, &client.get_user(), params, false).await,
        "NOTICE" if registered => msg(irc, &client.get_user(), params, true).await,
        "JOIN" if registered => join(irc, &client.get_user(), params).await,
        "PART" if registered => part(irc, &client.get_user(), params).await,
        "TOPIC" if registered => topic(irc, &client.get_user(), params).await,
        "LIST" if registered => list(irc).await,
        "PART" | "JOIN" | "PRIVMSG" | "NOTICE" | "TOPIC" | "LIST" if !registered => gef!(ircError::NotRegistered),
        _ => gef!(ircError::UnknownCommand(cmd)),
    }
}

pub async fn list(irc: &Core) -> Result<ClientReplies, GenError> {
    let tuple_vector = irc.get_list_reply();
    let mut replies = Vec::new();
    for (chan, topic) in tuple_vector.iter() {
        replies.push(Ok(ircReply::ListReply(chan.get_name(), chan.get_n_users(), topic.clone())));
    }
    replies.push(Ok(ircReply::EndofList));
    Ok(replies)
}

pub async fn topic(irc: &Core, user: &User, mut params: Vec<String>) -> Result<ClientReplies, GenError> {
    let mut replies = Vec::new();
    if params.is_empty() {
        replies.push(Err(ircError::NeedMoreParams("TOPIC".to_string())));
        return Ok(replies);
    }

    /* are ya in the chan? */
    let chanmask = params.remove(0);
    let chan = irc.get_chan(&chanmask)?;
    if !chan.is_joined(&user.get_nick()) {
        replies.push(Err(ircError::NotOnChannel(chanmask)));
        return Ok(replies);
    }

    /* just want to receive topic? */
    if params.is_empty() {
        if let Some(topic) = chan.get_topic() {
            replies.push(Ok(ircReply::Topic(chanmask.clone(), topic.text)));
            replies.push(Ok(ircReply::TopicSetBy(chanmask, topic.usermask, topic.timestamp)));
        } else {
            replies.push(Ok(ircReply::NoTopic(chanmask)));
        }
        return Ok(replies);
    };
    
    /* set topic IF permissions allow */
    if chan.is_op(user) {
        chan.set_topic(&params.remove(0), &user);
    } else {
        replies.push(Err(ircError::ChanOPrivsNeeded(chanmask)));
    }
    Ok(replies)
}

pub async fn join(irc: &Arc<Core>, user: &Arc<User>, mut params: Vec<String>) -> Result<ClientReplies, GenError> {
    let mut replies = Vec::new();
    if params.is_empty() {
        replies.push(Err(ircError::NeedMoreParams("JOIN".to_string())));
        return Ok(replies);
    }

    /* JOIN can take a second argument. The format is:
     * JOIN comma,sep.,chan,list comma,sep.,key,list
     * but I'll leave key implementation til later */
    let targets = params.remove(0);
    for target in targets.split(',') {
        replies.append(&mut irc.join_chan(&target, user).await?);
    }
    Ok(replies)
}

pub async fn part(irc: &Arc<Core>, user: &Arc<User>, mut params: Vec<String>) -> Result<ClientReplies, GenError> {
    let mut replies: ClientReplies = Vec::new();
    if params.is_empty() {
        replies.push(Err(ircError::NeedMoreParams("PART".to_string())));
        return Ok(replies);
    }

    let targets = params.remove(0);
    let part_msg = if params.is_empty() {
        String::from("")
    } else {
        params.remove(0)
    };
    for target in targets.split(',') {
        replies.push(irc.part_chan(&target, user, &part_msg).await);
    }
    Ok(replies)
}

pub async fn msg(
    irc: &Core,
    send_u: &Arc<User>,
    mut params: Vec<String>,
    notice: bool,
) -> Result<ClientReplies, GenError> {
    let mut replies = Vec::new();
    if params.is_empty() {
        if !notice {
                replies.push(Err(ircError::NoRecipient("PRIVMSG".to_string())));
        }
        return Ok(replies);
    }
    /* this appears to be what's crashing, despite the check for params.opt_params.is_empty() beforehand
     * ah, I'd forgotten to remove one of the notice bools from the above if statements,
     * if params.opt_params.is_empty() && notice won't work */
    let targets = params.remove(0); 
    let cmd = if notice { "NOTICE" } else { "PRIVMSG" };

    // if there were no more args, message should be an empty String
    if params.is_empty() {
        if !notice {
            replies.push(Err(ircError::NoTextToSend));
        }
        return Ok(replies);
    }
    // if there are more than two arguments,
    // concatenate the remainder to one string
    let message = params.join(" ");
    trace!("{} from user {} to {}, content: {}", cmd, send_u.get_nick(), targets, message);

    // loop over targets
    for target in targets.split(',') {
        match irc.get_name(target) {
            Some(NamedEntity::User(user_weak)) => {
                match User::upgrade(&user_weak, target) {
                    Ok(recv_u) => {
                        replies.push(recv_u.send_msg(&send_u, &cmd, &target, &message).await?);
                    },
                    Err(GenError::DeadUser(nick)) => {
                        let _res = irc.search_user_chans_purge(&nick);
                        if let Err(err) = irc.remove_name(&nick) {
                            warn!("error {} removing nick {} from hash, but it doesn't exist", err, &nick)
                        }
                    },
                    /* this may be a more serious error & will abort processing the join command */
                    Err(e) => return Err(e),
                }
            },
            Some(NamedEntity::Chan(chan))
                => replies.push(chan.send_msg(&send_u, &cmd, &target, &message).await?),
            None => replies.push(Err(ircError::NoSuchNick(target.to_string())))
        }
    }
    Ok(replies)
}

pub async fn user(irc: &Core, client: &Arc<Client>, params: Vec<String>) -> Result<ClientReplies, GenError> {
    // a USER command should have exactly four parameters
    // <username> <hostname> <servername> <realname>,
    // though we ignore the middle two unless a server is
    // forwarding the message
    let mut replies = Vec::new();
    if params.len() != 4 {
        return gef!(ircError::NeedMoreParams("USER".to_string()));
    }
    let username = params[0].clone();
    let real_name = params[3].clone();

    let result = match client.get_client_type() {
        ClientType::Dead => None,
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
            replies.push(Err(ircError::AlreadyRegistred));
            return Ok(replies);
        }
        ClientType::ProtoUser(proto_user_ref) => {
            // got nick already? if so, complete registration
            let proto_user = proto_user_ref.lock().unwrap();
            if let Some(nick) = &proto_user.nick {
                // had nick already, complete registration
                let ret = Some(ClientType::User(
                    irc.register(client, nick.clone(), username.clone(), real_name)?, // propagate the error if it goes wrong
                ));
                replies.push(Ok(ircReply::Welcome(nick.clone(), username.clone(), client.get_host_string())));
                replies.push(Ok(ircReply::YourHost(irc.get_host(), irc.get_version())));
                replies.push(Ok(ircReply::Created(irc.get_date())));
                replies.push(Ok(ircReply::MyInfo(irc.get_host(), irc.get_version(), irc.get_umodes(), irc.get_chanmodes())));
                ret
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
    Ok(replies)
}

pub async fn nick(irc: &Core, client: &Arc<Client>, params: Vec<String>) -> Result<ClientReplies, GenError> {
    let mut replies = Vec::new();
    let nick;
    if let Some(n) = params.iter().next() {
        nick = n.to_string();
    } else {
        replies.push(Err(ircError::NeedMoreParams("NICK".to_string())));
        return Ok(replies);
    }

    // is the nick a valid nick string?
    if !rfc::valid_nick(&nick) {
        replies.push(Err(ircError::ErroneusNickname(nick)));
        return Ok(replies);
    }

    // is this nick already taken?
    if let Some(_hit) = irc.get_name(&nick) {
        replies.push(Err(ircError::NicknameInUse(nick)));
        return Ok(replies);
    }

    // we can return a tuple and send messages after the match
    // to avoid borrowing mutably inside the immutable borrow
    // (Some(&str), Some(ClientType), bool died)
    let result = match client.get_client_type() {
        ClientType::Dead => None,
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
            user_ref.change_nick(&nick)?;
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
                let ret = Some(ClientType::User(
                    irc.register(
                        client,
                        nick.clone(),
                        username.unwrap().to_string(),
                        real_name.unwrap().to_string(),
                    )?, // error propagation if registration fails
                ));
                replies.push(Ok(ircReply::Welcome(nick.clone(), username.unwrap().clone(), client.get_host_string())));
                replies.push(Ok(ircReply::YourHost(irc.get_host(), irc.get_version())));
                replies.push(Ok(ircReply::Created(irc.get_date())));
                replies.push(Ok(ircReply::MyInfo(irc.get_host(), irc.get_version(), irc.get_umodes(), irc.get_chanmodes())));
                ret
            }
        }
    };

    if let Some(new_client_type) = result {
        client.set_client_type(new_client_type);
    }
    Ok(replies)
}
