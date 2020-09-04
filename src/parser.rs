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
use crate::irc::rfc_defs as rfc;
use std::{error, fmt};

#[derive(Debug)]
pub enum ParseError {
    NoCommand,
    InvalidCommand(String),
    InvalidNick(String),
    InvalidUser(String),
    InvalidHost(String),
    EmptyMessage,
    EmptyName,
    EmptyNick,
    EmptyHost,
    EmptyUser,
}

impl error::Error for ParseError {}

pub enum HostType {
    HostName(String),
    HostAddrV4(String),
    HostAddrV6(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::EmptyName => write!(f, "Empty name (nick/host) field `: CMD`"),
            ParseError::EmptyNick => write!(f, "Empty nick field `:!user@host CMD`"),
            ParseError::EmptyUser => write!(f, "Empty user field `:nick!@host CMD`"),
            ParseError::EmptyHost => write!(f, "Empty host field `:nick!user@ CMD`"),
            ParseError::EmptyMessage => write!(f, "Empty message"),
            ParseError::NoCommand => write!(f, "No command given"),
            ParseError::InvalidCommand(cmd) => write!(f, "Invalid command string: {}", &cmd),
            ParseError::InvalidNick(nick) => write!(f, "Invalid nick: {}", &nick),
            ParseError::InvalidUser(user) => write!(f, "Invalid user string: {}", &user),
            ParseError::InvalidHost(host) => write!(f, "Invalid host string: {}", &host),
        }
    }
}

pub enum MsgPrefix {
    Name(String), // generic for when we don't know if a name is a nickname or a hostname - special case
    Nick(String), // for when we can guess it's a nick and not a host, but have no other info
    NickHost(String, HostType),
    NickUserHost(String, String, HostType),
    Host(HostType),
}

pub struct ParsedMsg {
    pub opt_prefix: Option<MsgPrefix>,
    pub command: String,
    // NB: our parser first makes a Vec<&str>, where things will still point to stuff
    // in whatever the message slice sent to parse_message() was given a borrow of
    // why wrap a Vec with Option when you can just check whether it's empty?
    pub opt_params: Vec<String>,
}

// This code is terrible, gonna rewrite it completely
// What we are expecting is a line of text with no CR LF
// Use iterators to tokenize on SPACE but note also
// the position of the first " :" -- important
//    Augmented BNF notation for general message strcture
//    message    =  [ ":" prefix SPACE ] command [ params ]
pub fn parse_message(message: &str) -> Result<ParsedMsg, ParseError> {
    let mut line = message;
    if line.is_empty() {
        return Err(ParseError::EmptyMessage);
    }
    let opt_prefix = if &message[..1] == ":" {
        // try for prefix
        let vec: Vec<&str> = line.splitn(2, ' ').collect();
        if vec.len() < 2 {
            return Err(ParseError::NoCommand);
        }
        line = vec[1];
        Some(parse_prefix(&vec[0])?)
    } else {
        None
    };

    let mut params: Vec<String> = Vec::new();
    let mut n_args = 0;
    loop {
        let vec: Vec<&str> = line.splitn(2, ' ').collect();
        n_args += 1;
        params.push(vec[0].to_string());
        if vec.len() < 2 {
            break;
        }

        line = vec[1];
        // " :" means squash/collect all remaining args,
        // which is also supposed to happen if rfc::MaxParams
        // is reached
        if line.is_empty() {
            break;
        } else if &line[..1] == ":" {
            line = &line[1..line.len()];
            params.push(line.to_string());
            break;
        } else if n_args >= 16 {
            params.push(line.to_string());
            break;
        }
    }
    /* should be safe - above code ensure non-zero length of params */
    let command = params.remove(0);

    // return the stuff
    Ok(ParsedMsg {
        opt_prefix,
        command,
        opt_params: params,
    })
}

// parse the prefix part of an IRC message
// with preceding colon and delimiting space stripped off
fn parse_prefix(msg: &str) -> Result<MsgPrefix, ParseError> {
    // start over with this...,
    // first, let's tokenize with '@'
    let first_split: Vec<&str> = msg.splitn(2, '@').collect();
    let name: &str = first_split[0];
    if name.is_empty() { return Err(ParseError::EmptyName); }

    if first_split.len() == 2 {
        let host = first_split[1];
        if host.is_empty() { return Err(ParseError::EmptyHost); }
        // in this case we must have some sort of nick@host or possibly nick!user@host type
        // thing, so let's deal with that first...
        let second_split: Vec<&str> = first_split[0].splitn(2, '!').collect();
        if second_split.len() == 2 {
            let (nick, user) = (second_split[0].to_string(), second_split[1].to_string());
            if nick.is_empty() { return Err(ParseError::EmptyNick); }
            if user.is_empty() { return Err(ParseError::EmptyUser); }
            if !rfc::valid_user(&user) {
                Err(ParseError::InvalidUser(user))
            } else if !rfc::valid_nick(&nick) {
                Err(ParseError::InvalidNick(nick))
            } else {
                Ok(MsgPrefix::NickUserHost(nick, user, parse_host(host)?))
            }
        } else {
            let nick = name.to_string();
            if !rfc::valid_nick(&nick) {
                Err(ParseError::InvalidNick(nick))
            } else {
                Ok(MsgPrefix::NickHost(nick, parse_host(host)?))
            }
        }
    } else if !rfc::valid_nick(name) {
        // server case
        Ok(MsgPrefix::Host(parse_host(name)?)) // we got a host :D
    } else {
        // if we didn't get an @, and the nick is valid
        // we can't actually be totally sure if we have a
        // nick or a host - tho we could rule out host with additional checks i suppose
        // in this case we keep the match thing because we don't actually want to
        // treat this "error" as an error
        match parse_host(name) {
            Ok(_) => Ok(MsgPrefix::Name(name.to_string())), // valid as host OR nick
            Err(_) => Ok(MsgPrefix::Nick(name.to_string())), // only valid as nick
        }
    }
}

// this host parsing code will assign whether we have a regular hostname (and if it's valid),
// or an ipv4/ipv6 address
// decided not to use net::IpAddr here in the parser,
// addresses may possibly be converted into proper formats elsewhere if needed
fn parse_host(host_string: &str) -> Result<HostType, ParseError> {
    let host = host_string.to_string();
    if rfc::valid_ipv4_addr(&host) {
        Ok(HostType::HostAddrV4(host))
    } else if rfc::valid_ipv6_addr(&host) {
        Ok(HostType::HostAddrV6(host))
    } else if rfc::valid_hostname(&host) {
        Ok(HostType::HostName(host))
    } else {
        Err(ParseError::InvalidHost(host))
    }
}
