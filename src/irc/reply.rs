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

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reply::None => write!(f, "300"),
            Reply::ListReply(chan, topic) => write!(f, "322 :{} {}", chan, topic),
            Reply::EndofList => write!(f, "323 :End of /LIST"),
            Reply::Topic(chan, topic_msg) => write!(f, "332 {} :{}", chan, topic_msg),
            Reply::NameReply(chan, nicks) => write!(f, "353 {} :{}", chan, nicks.join(" ")),
            Reply::EndofNames(chan) => write!(f, "366 {} :End of /NAMES list", chan),
        }
    }
}

pub enum Reply {
    None,
    Topic(String, String),
    NameReply(String, Vec<String>),
    ListReply(String, String),
    EndofList,
    EndofNames(String),
}
