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
use std::{error, fmt};
use std::sync::{Arc, Mutex, Weak};

use log::debug;

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

    /* generate a vector of Arc pointers to users on this channel,
     * remove any nicks from the tree if upgrade on the weak pointer
     * fails */
    pub fn gen_user_ptr_vec(&self) -> Vec<Arc<User>> {
        let mut locked = self.users.lock().unwrap();
        locked.iter()
            .filter_map(|(key, val)|{
                if let Some(ptr) = Weak::upgrade(&val.user_ptr) {
                    Some(ptr)
                } else {
                    /* with iterator style it also seems
                     * possible to remove the bad keys on the fly
                     */
                    debug!("remove bad key {} from chan {}", key, self.get_name());
                    locked.remove(&key.clone());
                    if locked.is_empty() {
                        debug!("{} empty, unlink from irc::Core HashMap", self.get_name());
                        self.irc.remove_name(&self.get_name());
                    }; None
                }
        }).collect::<Vec<_>>()
    }

    /* spit out a vector of (key, value) tuples */
    fn _get_user_list(&self) -> Vec<(String, ChanUser)> {
        self.users
            .lock()
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>()
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
    pub fn add_user(self: &Arc<Self>, new_user: &Arc<User>, flags: ChanFlags) -> Result<(), ChanError> {
        let mut chan_mutex_lock = self.users.lock().unwrap();
        let mut user_mutex_lock = new_user.channel_list.lock().unwrap();
        let nick = new_user.get_nick();
        let chan = self.get_name();
        let chan_ptr = Arc::downgrade(&self);

        if !chan_mutex_lock.contains_key(&nick) {
            chan_mutex_lock.insert(nick, ChanUser::new(new_user, flags));
            user_mutex_lock.insert(chan, chan_ptr);
            Ok(())
        } else {
            Err(ChanError::LinkFailed(nick, chan))
        }
    }

    /* put add_ and rm_user() here together and have all the code to handle
     * that in one place, both for User and Chan side - plus, mutex lock
     * everything for the entire fn call */
    pub fn rm_user(&self, user: &User) -> Result<(), ChanError> {
        let mut chan_mutex_lock = self.users.lock().unwrap();
        let mut user_mutex_lock = user.channel_list.lock().unwrap();

        let key = user.get_nick().to_string();
        let chan = self.get_name();
        if let Some(val) = chan_mutex_lock.remove(&key) {
            user_mutex_lock.remove(&chan); Ok(())
        } else {
            Err(ChanError::UnlinkFailed(key, chan))
        }
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
