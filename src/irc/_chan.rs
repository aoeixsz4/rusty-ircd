extern crate log;
extern crate chrono;
use crate::client::GenError;
use crate::irc::{Core, User};

use chrono::Utc;
use std::clone::Clone;
use std::collections::BTreeMap;
use std::{error, fmt};
use std::sync::{Arc, Mutex, Weak};

use log::{debug,warn};

#[derive(Debug)]
pub enum ChanError {
    LinkFailed(String, String),
    UnlinkFailed(String, String),
}

impl error::Error for ChanError {}
impl fmt::Display for ChanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChanError::LinkFailed(nick, chan) => write!(f, "couldn't add {} to {} channel list", nick, chan),
            ChanError::UnlinkFailed(nick, chan) => write!(f, "couldn't remove {} from {} channel list", nick, chan),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChanFlags {
    None,
    Voice,
    Op,
}

#[derive(Debug, Clone)]
pub struct ChanUser {
    user_ptr: Weak<User>,
    chan_flags: ChanFlags,
}

impl ChanUser {
    pub fn new(user: &Arc<User>, flags: ChanFlags) -> ChanUser {
        ChanUser {
            user_ptr: Arc::downgrade(&user),
            chan_flags: flags,
        }
    }
}

#[derive(Debug)]
pub struct ChanTopic {
    pub text: String,
    pub usermask: String,
    pub timestamp: i64
}

impl Clone for ChanTopic {
    fn clone(&self) -> Self {
        ChanTopic {
            text: self.text.clone(),
            usermask: self.usermask.clone(),
            timestamp: self.timestamp
        }
    }
}

#[derive(Debug)]
pub struct Channel {
    name: String,
    topic: Mutex<Option<ChanTopic>>,
    users: Mutex<BTreeMap<String, ChanUser>>,
    banmasks: Mutex<Vec<String>>,
    irc: Arc<Core>,
}

impl Channel {
    pub fn new(irc: &Arc<Core>, chanmask: &str) -> Channel {
        let name = chanmask.to_string();
        let topic = Mutex::new(None);
        let users = Mutex::new(BTreeMap::new());
        let banmasks = Mutex::new(Vec::new());
        Channel {
            name,
            topic,
            users,
            banmasks,
            irc: Arc::clone(&irc)
        }
    }

    /* spit out a vector of (key, value) tuples */
    fn _get_user_list(&self) -> Vec<(String, ChanUser)> {
        self.users
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .collect::<Vec<_>>()
    }

    /* generate a vector of Arc pointers to users on this channel,
     * remove any nicks from the tree if upgrade on the weak pointer
     * fails */
    pub fn gen_user_ptr_vec(&self) -> Vec<Arc<User>> {
        let mut bad_keys = Vec::new();
        let mut ret = Vec::new();
        for (key, val) in self._get_user_list().iter() {
            if let Some(ptr) = Weak::upgrade(&val.user_ptr) {
                ret.push(ptr);
            } else {
                bad_keys.push(key.clone());
            }
        }
        for key in bad_keys.iter() {
            self.users.lock().unwrap().remove(key);
        }
        ret
    }

    /* this one just gives the actual nicks themselves,
     * without chan privilege signifiers */
    fn _get_nick_list_wo_badges(&self) -> Vec<String> {
        self._get_user_list()
            .iter()
            .map(|(key, _val)|{
                key.to_string()
            }).collect::<Vec<_>>()
    }

    /* this time give the nicks processed with added '+'
     * tag for voice or '@' for chanop */
    pub fn get_nick_list(&self) -> Vec<String> {
        self._get_user_list()
            .iter()
            .map(|(key, val)| {
                match val.chan_flags {
                    ChanFlags::None => key.to_string(),
                    ChanFlags::Voice => format!("+{}", key).to_string(),
                    ChanFlags::Op => format!("@{}", key).to_string(),
                }
            }).collect::<Vec<_>>()
    }

    pub fn get_n_users(&self) -> usize {
        self.users.lock().unwrap().len()
    }

    pub fn get_topic(&self) -> Option<ChanTopic> {
        match self.topic.lock().unwrap().clone() {
            Some(topic) => Some(topic.clone()),
            None => None
        }
    }

    pub fn set_topic(&self, topic_text: &str, user: &User) {
        let topic = ChanTopic {
            text: topic_text.to_string(),
            usermask: user.get_prefix(),
            timestamp: Utc::now().timestamp()
        };
        *self.topic.lock().unwrap() = Some(topic);
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_names_list(&self) -> Vec<String> {
        self.get_nick_list()
    }

    pub fn is_empty(&self) -> bool {
        self.users.lock().unwrap().is_empty()
    }

    pub fn is_op(&self, user: &User) -> bool {
        let op = format!("@{}", &user.nick.lock().unwrap());
        self.users.lock().unwrap().contains_key(&op)
    }

    pub fn is_joined(&self, nick: &str) -> bool {
        self.users.lock().unwrap().contains_key(nick)
    }

    /* put add_ and rm_user() here together and have all the code to handle
     * that in one place, both for User and Chan side - plus, mutex lock
     * everything for the entire fn call */
    pub async fn add_user(self: &Arc<Self>, new_user: &Arc<User>, flags: ChanFlags) -> Result<ClientReplies, GenError> {
        let chan = self.get_name();
        let mut replies = Vec::new();
        {
            let mut chan_mutex_lock = self.users.lock().unwrap();
            let mut user_mutex_lock = new_user.channel_list.lock().unwrap();
            let nick = new_user.get_nick();
            let chan = self.get_name();
            let chan_ptr = Arc::downgrade(&self);

            if !chan_mutex_lock.contains_key(&nick) {
                chan_mutex_lock.insert(nick, ChanUser::new(new_user, flags));
                user_mutex_lock.insert(chan, chan_ptr);

                
            } else {
                return Ok(replies) /* already on chan */
            }
        } /* de-scope mutex locks */

        /* also self.notify_join() */
        replies.push(self.notify_join(new_user, &chan).await?);
        if let Some(topic) = self.get_topic() {
            replies.push(Ok(ircReply::Topic(chan.to_string(), topic.text)));
            replies.push(Ok(ircReply::TopicSetBy(chan.to_string(), topic.usermask, topic.timestamp)))
        }
        replies.push(Ok(ircReply::NameReply(chan.to_string(), self.get_nick_list())));
        replies.push(Ok(ircReply::EndofNames(chan.to_string())));
        Ok(replies)
    }

    /* still need this for User::drop() */
    pub fn rm_key(&self, key: &str) -> Option<ChanUser> {
        self.users.lock().unwrap().remove(key)
    }

    /* put add_ and rm_user() here together and have all the code to handle
     * that in one place, both for User and Chan side - plus, mutex lock
     * everything for the entire fn call */
    pub async fn rm_user(&self, user: &User, msg: &str) -> Result<(), ChanError> {
        /* Notify part msg */
        if !self.is_empty() {
            let _res = self.notify_part(user, &self.get_name(), msg).await;
        }

        let retval = {
            let mut chan_mutex_lock = self.users.lock().unwrap();
            let mut user_mutex_lock = user.channel_list.lock().unwrap();

            let key = user.get_nick().to_string();
            let chan = self.get_name();
            if let Some(_val) = chan_mutex_lock.remove(&key) {
                user_mutex_lock.remove(&chan);
                if chan_mutex_lock.is_empty() {
                    if let Err(err) = self.irc.remove_name(&chan) {
                        warn!("error {} removing chan {} from hash - it doesn't exist", err, &chan);
                    }
                }
                Ok(())
            } else {
                Err(ChanError::UnlinkFailed(key, chan))
            }
        }; /* de-scope Mutex */

        retval
    }

    /* similar rationale to the above about linking and unlinking users to chans */
    pub fn update_nick(&self, old_nick: &str, new_nick: &str) -> Result<(), ircError> {
        let mut mutex_lock = self.users.lock().unwrap();
        let key = old_nick.to_string();
        if let Some(val) = mutex_lock.remove(&key) {
            mutex_lock.insert(new_nick.to_string(), val);
            Ok(())
        } else {
            Err(ircError::NotOnChannel(self.name.clone()))
        }
    }

    async fn _send_msg(
        &self,
        source: &User,
        command_str: &str,
        target: &str,
        msg: &str
    ) -> Result<ClientReply, GenError> {
        // checks for banmasks should be done-
        // also whether the sending user is in the channel or not
        let prefix = source.get_prefix();
        let line = if msg.is_empty() {
            format!(":{} {} {}", prefix, command_str, target)
        } else {
            format!(":{} {} {} :{}", prefix, command_str, target, msg)
        };

        if self.is_joined(&source.get_nick()) {
            // if we clone the list, the true list could change while
            // we're forwarding messages, but this keeps us thread safe
            let users = self.gen_user_ptr_vec();
            for user in users.iter() {
                // if you're parting or joining, your own echoed message confirms success
                if user.id != source.id || command_str == "JOIN" || command_str == "PART" {
                    if let Err(err) = user.send_line(&line).await {
                        debug!("another tasks's client died: {}, note dead key {}", err, &user.get_nick());
                        //user.clear_chans_and_exit();
                    }
                }
            }
            Ok(Ok(ircReply::None))
        } else {
            Ok(Err(ircError::CannotSendToChan(target.to_string())))
        }
    }

    pub async fn send_msg(&self, source: &User, cmd: &str, target: &str, msg: &str) -> Result<ClientReply, GenError> {
        self._send_msg(source, cmd, target, msg).await
    }

    pub async fn notify_join(&self, source: &User, chan: &str) -> Result<ClientReply, GenError> {
        self._send_msg(source, "JOIN", chan, "").await
    }

    pub async fn notify_part(&self, source: &User, chan: &str, msg: &str) -> Result<ClientReply, GenError> {
        self._send_msg(source, "PART", chan, msg).await
    }

    pub async fn notify_quit(&self, source: &User, chan: &str, msg: &str) -> Result<ClientReply, GenError> {
        self._send_msg(source, "QUIT", chan, msg).await
    }
}
