macro_rules! gef {
    ($e:expr) => (Err(GenError::from($e)));
}

extern crate log;
use crate::client::GenError;
use crate::irc::error::Error as ircError;
use crate::irc::reply::Reply as ircReply;
use crate::irc::{Core, User};

use std::clone::Clone;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use log::debug;

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
pub struct Channel {
    name: String,
    topic: Mutex<String>,
    users: Mutex<BTreeMap<String, ChanUser>>,
    banmasks: Mutex<Vec<String>>,
    irc: Arc<Core>,
}

impl Channel {
    pub fn new(irc: &Arc<Core>, chanmask: &str) -> Channel {
        let name = chanmask.to_string();
        let topic = Mutex::new(String::from(""));
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

    pub fn add_user(&self, new_user: &Arc<User>, flags: ChanFlags) {
        let nick_str = match flags {
            ChanFlags::None => new_user.get_nick(),
            ChanFlags::Voice => format!("+{}", new_user.get_nick()),
            ChanFlags::Op => format!("@{}", new_user.get_nick()),
        };
        self.users
            .lock()
            .unwrap()
            .entry(nick_str)
            .or_insert_with(|| ChanUser::new(new_user, flags));
    }

    pub fn gen_user_ptr_vec(&self) -> Vec<Arc<User>> {
        let mut users = Vec::new();
        let cloned = self.users.lock().unwrap().clone();
        for (key, val) in cloned.iter() {
            if let Some(arc_ptr) = Weak::upgrade(&val.user_ptr) {
                users.push(arc_ptr);
            } else {
                debug!("chan {}: could not upgrade pointer to user under key {}",
                        self.name, key);
                self.rm_key(key);
                debug!("chan {}: remove key {}", self.name, key);
                if self.is_empty() {
                    debug!("chan {} empty, remove from IRC HashMap", self.name);
                    self.irc.remove_name(&self.name);
                }
            }
        }
        users
    }

    pub fn gen_sorted_nick_list(&self) -> Vec<String> {
        let mut ret = Vec::new();
        let locked = self.users.lock().unwrap().clone();
        for key in locked.keys() {
            ret.push(key.clone());
        }
        ret
    }

    // this bit is rather inefficient...
    pub fn get_user_key(&self, nick: &str) -> Option<String> {
        let key = nick.to_string();
        let voice = format!("+{}", key);
        let op = format!("@{}", key);
        if self.users.lock().unwrap().contains_key(&key) {
            Some(key)
        } else if self.users.lock().unwrap().contains_key(&voice) {
            Some(voice)
        } else if self.users.lock().unwrap().contains_key(&op) {
            Some(op)
        } else {
            None
        }
    }

    pub fn get_topic(&self) -> String {
        self.topic.lock().unwrap().to_string()
    }

    pub fn set_topic(&self, topic: &str) {
        *self.topic.lock().unwrap() = topic.to_string()
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_names_list(&self) -> Vec<String> {
        self.gen_sorted_nick_list()
    }

    pub fn is_empty(&self) -> bool {
        self.users.lock().unwrap().is_empty()
    }

    pub fn is_op(&self, user: &User) -> bool {
        let op = format!("@{}", &user.nick.lock().unwrap());
        self.users.lock().unwrap().contains_key(&op)
    }

    pub fn is_joined(&self, nick: &str) -> bool {
        let names_list = self.get_names_list();
        for name in names_list.iter() {
            if
            (!name.is_empty()
                && (&name[..1] == "+" || &name[..1] == "@")
                && nick == &name[1..]
            ) || nick == &name[..] {
                return true;
            }
        }
        false
    }

    pub fn rm_key(&self, key: &str) {
        self.users.lock().unwrap().remove(key);
        let voice = format!("+{}", key);
        let op = format!("@{}", key);
        if !key.is_empty() && &key[..1] != "+" && &key[..1] != "@" {
            self.users.lock().unwrap().remove(&voice);
            self.users.lock().unwrap().remove(&op);
        }
    }

    pub fn update_nick(&self, old_nick: &str, new_nick: &str) -> Result<(), ircError> {
        let mut mutex_lock = self.users.lock().unwrap();
        let key = old_nick.to_string();
        let voice = format!("+{}", key);
        let op = format!("@{}", key);
        if let Some(val) = mutex_lock.remove(&key) {
            mutex_lock.insert(key, val);
            Ok(())
        } else if let Some(val) = mutex_lock.remove(&voice) {
            let new_key = format!("+{}", new_nick);
            mutex_lock.insert(new_key, val);
            Ok(())
        } else if let Some(val) = mutex_lock.remove(&voice) {
            let new_key = format!("@{}", new_nick);
            mutex_lock.insert(new_key, val);
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
    ) -> Result<ircReply, GenError> {
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
                if user.id != source.id {
                    if let Err(err) = user.send_line(&line).await {
                        debug!("another tasks's client died: {}, note dead key {}", err, &user.get_nick());
                        //user.clear_chans_and_exit();
                    }
                }
            }
            Ok(ircReply::None)
        } else {
            gef!(ircError::CannotSendToChan(target.to_string()))
        }
    }

    pub async fn send_msg(&self, source: &User, cmd: &str, target: &str, msg: &str) -> Result<ircReply, GenError> {
        self._send_msg(source, cmd, target, msg).await
    }

    pub async fn notify_join(&self, source: &User, chan: &str) -> Result<ircReply, GenError> {
        self._send_msg(source, "JOIN", chan, "").await
    }

    pub async fn notify_part(&self, source: &User, chan: &str, msg: &str) -> Result<ircReply, GenError> {
        self._send_msg(source, "PART", chan, msg).await
    }

    pub async fn notify_quit(&self, source: &User, chan: &str, msg: &str) -> Result<ircReply, GenError> {
        self._send_msg(source, "QUIT", chan, msg).await
    }
}
