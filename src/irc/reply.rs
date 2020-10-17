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
/*
    300 RPL_NONE
    302 RPL_USERHOST ":[<reply>{<space><reply>}]"
    303 RPL_ISON ":[<nick> {<space><nick>}]"
    301 RPL_AWAY "<nick> :<away message>"
    305 RPL_UNAWAY ":You are no longer marked as being away"
    306 RPL_NOWAWAY ":You have been marked as being away"
    311 RPL_WHOISUSER "<nick> <user> <host> * :<real name>"
    312 RPL_WHOISSERVER "<nick> <server> :<server info>"
    313 RPL_WHOISOPERATOR "<nick> :is an IRC operator"
    317 RPL_WHOISIDLE "<nick> <integer> :seconds idle"
    318 RPL_ENDOFWHOIS "<nick> :End of /WHOIS list"
    319 RPL_WHOISCHANNELS "<nick> :{[@|+]<channel><space>}"
    314 RPL_WHOWASUSER "<nick> <user> <host> * :<real name>"
    369 RPL_ENDOFWHOWAS "<nick> :End of WHOWAS"
    321 RPL_LISTSTART "Channel :Users  Name"
    322 RPL_LIST "<channel> <# visible> :<topic>"
    323 RPL_LISTEND ":End of /LIST"
    324 RPL_CHANNELMODEIS "<channel> <mode> <mode params>"
    331 RPL_NOTOPIC "<channel> :No topic is set"
    332 RPL_TOPIC "<channel> :<topic>"
    341 RPL_INVITING "<channel> <nick>"
    342 RPL_SUMMONING "<user> :Summoning user to IRC"
    351 RPL_VERSION "<version>.<debuglevel> <server> :<comments>"
    352 RPL_WHOREPLY "<channel> <user> <host> <server> <nick> <H|G>[*][@|+] :<hopcount> <real name>"
    seems to be some missing...
*/

use std::fmt;
use std::str;
use crate::irc::rfc_defs as rfc;
use crate::irc::chan::ChanTopic;

pub enum Reply {
    None,
    Welcome(String, String, String),
    YourHost(String, String),
    Created(String),
    MyInfo(String, String, String, String),
    NoTopic(String),
    Topic(String, String),
    TopicSetBy(String, String, i64),
    NameReply(String, Vec<String>),
    EndofNames(String),
    ListStart,
    ListReply(String, usize, Option<ChanTopic>),
    EndofList,
}

type Code = u16;
type CodeStr = String;

impl Reply {
    /* map enums to numberic reply codes */
    fn numeric(&self) -> Code {
        match self {
            Reply::Welcome(_n, _u, _h) => 001,
            Reply::YourHost(_s,_v) => 002,
            Reply::Created(_t) => 003,
            Reply::MyInfo(_s, _v, _um, _cm) => 004,
            Reply::None => 300,
            Reply::ListStart => 321,
            Reply::ListReply(_ch, _nu, _top) => 322,
            Reply::EndofList => 323,
            Reply::NoTopic(_ch) => 331,
            Reply::Topic(_ch, _top) => 332,
            Reply::TopicSetBy(_ch, _umask, _stamp) => 333,
            Reply::NameReply(_ch, _ns) => 353,
            Reply::EndofNames(_ch) => 366
        }
    }

    /* convert reply codes to strings */
    fn reply_code(&self) -> CodeStr {
        format!("{:03}", self.numeric())
    }

    /* the body is everything in the reply after :<server> <Code> <recipient> */
    fn body(&self) -> Option<String> {
        match self {
            Reply::None => None,
            Reply::Welcome(nick, user, host) => Some(format!(":Welcome to Rusty IRC Network {}!{}@{}", nick, user, host)),
            Reply::YourHost(serv, ver) => Some(format!(":Your host is {}, running version {}", serv, ver)),
            Reply::Created(time) => Some(format!(":This server was created {}", time)),
            Reply::MyInfo(serv, ver, umodes, chanmodes) => Some(format!(":{} {} {} {}", serv, ver, umodes, chanmodes)),
            Reply::ListStart => Some(format!("Channel Users :Topic")),
            Reply::ListReply(chan, n_users, topic_opt) => {
                if let Some(topic) = topic_opt {
                    Some(format!("{} {} :{}", chan, n_users, topic.text))
                } else {
                    Some(format!("{} {}", chan, n_users))
                }
            },
            Reply::EndofList => Some(format!(":End of /LIST")),
            Reply::NoTopic(chan) => Some(format!("{} :No topic is set.", chan)),
            Reply::Topic(chan, topic_msg) => Some(format!("{} :{}", chan, topic_msg)),
            Reply::TopicSetBy(chan, usermask, timestamp) => Some(format!("{} {} {}", chan, usermask, timestamp)),
            Reply::NameReply(chan, nicks) => Some(format!("{} :{}", chan, nicks.join(" "))),
            Reply::EndofNames(chan) => Some(format!("{} :End of /NAMES list", chan)),
        }
    }

    /* format a full IRC string for sending to the client
       - NB this isn't currently checked for exceeding RFC message length */
    pub fn format(&self, server: &str, recipient: &str) -> String {
        if let Some(reply_body) = self.body() {
            format!(":{} {} {} {}", server, self.reply_code(), recipient, reply_body)
        } else {
            format!(":{} {} {}", server, self.reply_code(), recipient)
        }
    }
}

/* `:asdf.cool.net 001 luser :Welcome my lovely!` */
pub fn split(message: &str) -> (String, Option<String>) {
    let msg_bytes = message.as_bytes();
    if msg_bytes.len() <= rfc::MAX_MSG_SIZE - 2
        || msg_bytes[0] != b':' {
        return (message.to_string(), None);
    }
    
    let message_trimmed = &message[1..];
    let substrings: Vec<&str> = message_trimmed.splitn(2, " :").collect();
    if substrings.len() != 2 {
        panic!("message {} contains no ` :` token!", message);
    }
    let prefix = substrings[0];
    let prefix_bytes = substrings[0].as_bytes();
    let reply_bulk = substrings[1].as_bytes();
    let overhead = prefix_bytes.len() + 5;
    let room = rfc::MAX_MSG_SIZE - overhead;
    if reply_bulk.len() <= room {
        panic!("body {} is already short enough, algorithm is broken", str::from_utf8(reply_bulk).unwrap());
    }

    if let Some(space_index) = rfind_space_index(reply_bulk, room) {
        let chunk = str::from_utf8(&reply_bulk[..space_index]).unwrap();
        let remainder = str::from_utf8(&reply_bulk[space_index+1..]).unwrap();
        (
            format!(":{} :{}", prefix, chunk),
            Some(format!(":{} :{}", prefix, remainder))
        )
    } else {
        /* if there was no space we could use to split at, just cut arbitrarily at the max */
        let chunk = str::from_utf8(&reply_bulk[..room]).unwrap();
        let remainder = str::from_utf8(&reply_bulk[room..]).unwrap();
        (
            format!(":{} :{}", prefix, chunk),
            Some(format!(":{} :{}", prefix, remainder))
        )
    }
}

fn rfind_space_index (bytes: &[u8], mut index: usize) -> Option<usize> {
    while index > 0 {
        if bytes[index] == b' ' {
            return Some(index);
        }
        index -= 1;
    }
    None
}

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reply::None => write!(f, "300"),
            Reply::Welcome(nick, user, host) => write!(f, "001 :Welcome to Rusty IRC Network {}!{}@{}", nick, user, host),
            Reply::YourHost(serv, ver) => write!(f, "002 :Your host is {}, running version {}", serv, ver),
            Reply::Created(time) => write!(f, "003 :This server was created {}", time),
            Reply::MyInfo(serv, ver, umodes, chanmodes) => write!(f, "004 :{} {} {} {}", serv, ver, umodes, chanmodes),
            Reply::ListStart => write!(f, "321 Chan Users :Topic"),
            Reply::ListReply(chan, n_users, topic_opt) => {
                if let Some(topic) = topic_opt {
                    write!(f, "322 {} {} :{}", chan, n_users, topic.text)
                } else {
                    write!(f, "322 {} {}", chan, n_users)
                }
            },
            Reply::EndofList => write!(f, "323 :End of /LIST"),
            Reply::NoTopic(chan) => write!(f, "331 {} :No topic is set", chan),
            Reply::Topic(chan, topic_msg) => write!(f, "332 {} :{}", chan, topic_msg),
            Reply::TopicSetBy(chan, usermask, timestamp) => write!(f, "333 {} {} {}", chan, usermask, timestamp),
            Reply::NameReply(chan, nicks) => write!(f, "353 {} :{}", chan, nicks.join(" ")),
            Reply::EndofNames(chan) => write!(f, "366 {} :End of /NAMES list", chan),
        }
    }
}