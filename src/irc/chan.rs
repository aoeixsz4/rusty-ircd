use crate::client::GenError;
use crate::irc::error::Error as ircError;
use crate::irc::{MsgType, User};

use std::clone::Clone;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

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
}

impl Channel {
    pub fn new(chanmask: &str) -> Channel {
        let name = chanmask.to_string();
        let topic = Mutex::new(String::from(""));
        let users = Mutex::new(BTreeMap::new());
        let banmasks = Mutex::new(Vec::new());
        Channel {
            name,
            topic,
            users,
            banmasks,
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
        let mut ret = Vec::new();
        let locked = self.users.lock().unwrap();
        for val in locked.values() {
            ret.push(Weak::upgrade(&val.user_ptr).unwrap());
        }
        ret
    }

    pub fn gen_sorted_nick_list(&self) -> Vec<String> {
        let mut ret = Vec::new();
        let locked = self.users.lock().unwrap();
        for key in locked.keys() {
            ret.push(key.clone());
        }
        ret
    }

    // this bit is rather inefficient...
    pub fn get_user_key(&self, user: &User) -> Option<String> {
        let key = user.get_nick();
        if self.users.lock().unwrap().contains_key(&key) {
            return Some(key);
        }
        let voice = format!("+{}", key);
        if self.users.lock().unwrap().contains_key(&voice) {
            return Some(voice);
        }
        let op = format!("@{}", key);
        if self.users.lock().unwrap().contains_key(&op) {
            return Some(op);
        }
        None
    }

    pub fn get_topic(&self) -> String {
        self.topic.lock().unwrap().to_string()
    }

    pub fn is_empty(&self) -> bool {
        self.users.lock().unwrap().is_empty()
    }

    pub fn is_joined(&self, sought_user: &User) -> bool {
        let user_list = self.gen_user_ptr_vec();
        for user in user_list.iter() {
            if user.id == sought_user.id {
                return true;
            }
        }
        false
    }

    pub fn rm_key(&self, key: &str) {
        self.users.lock().unwrap().remove(key);
    }

    pub async fn notify_join(&self, source: &User, target: &str) -> Result<(), GenError> {
        // checks for banmasks should be done-
        // also whether the sending user is in the channel or not
        let prefix = source.get_prefix();
        let command_str = "JOIN";
        let line = format!(":{} {} {}", prefix, command_str, target);
        // if we clone the list, the true list could change while
        // we're forwarding messages, but this keeps us thread safe
        let user_list = self.gen_user_ptr_vec();
        for user in user_list.iter() {
            if user.id != source.id {
                let result = user.send_line(&line).await;
                if let Err(err) = result {
                    println!("another tasks's client died: {}", err);
                }
            }
        }
        Ok(())
    }

    pub async fn notify_part(
        &self,
        source: &User,
        target: &str,
        part_msg: &str,
    ) -> Result<(), GenError> {
        // checks for banmasks should be done-
        // also whether the sending user is in the channel or not
        let prefix = source.get_prefix();
        let command_str = "PART";
        let line = format!(":{} {} {} :{}", prefix, command_str, target, part_msg);
        // if we clone the list, the true list could change while
        // we're forwarding messages, but this keeps us thread safe
        let user_list = self.gen_user_ptr_vec();
        for user in user_list.iter() {
            let result = user.send_line(&line).await;
            if let Err(err) = result {
                println!("another tasks's client died: {}", err);
            }
        }
        Ok(())
    }

    pub async fn send_msg(
        &self,
        source: &User,
        target: &str,
        msg: &str,
        msg_type: &MsgType,
    ) -> Result<(), ircError> {
        // checks for banmasks should be done-
        // also whether the sending user is in the channel or not
        let prefix = source.get_prefix();
        let command_str = match msg_type {
            MsgType::PrivMsg => "PRIVMSG",
            MsgType::Notice => "NOTICE",
        };
        let line = format!(":{} {} {} :{}", prefix, command_str, target, msg);
        if self.is_joined(source) {
            // if we clone the list, the true list could change while
            // we're forwarding messages, but this keeps us thread safe
            let user_list = self.gen_user_ptr_vec();
            for user in user_list.iter() {
                if user.id != source.id {
                    let result = user.send_line(&line).await;
                    if let Err(err) = result {
                        println!("another tasks's client died: {}", err);
                    }
                }
            }
            Ok(())
        } else {
            Err(ircError::CannotSendToChan(target.to_string()))
        }
    }
}
