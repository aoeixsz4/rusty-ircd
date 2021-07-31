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
use crate::irc::rfc_defs as rfc;
use std::collections::HashMap;
use std::iter::Peekable;
use std::{error, fmt};
use std::str::{Chars, FromStr, ParseBoolError};

use super::rfc_defs::valid_nick;

#[derive(Debug)]
pub enum ParseError {
    NoCommand,
    InvalidCommand(String),
    InvalidNick(String),
    InvalidUser(String),
    InvalidHost(String),
    InvalidNickOrHost(String),
    EmptyMessage,
    EmptyName,
    EmptyNick,
    EmptyHost,
    EmptyUser,
}

impl error::Error for ParseError {}

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
            ParseError::InvalidNickOrHost(name) => write!(f, "Neither valid nick nor hostname: {}", &name),
        }
    }
}


#[derive(Debug)]
pub enum HostType {
    HostName(String),
    HostAddrV4(String),
    HostAddrV6(String),
}

pub type Tags = HashMap<String, Option<String>>;

#[derive(Debug)]
pub enum Prefix {
    Name(String), // generic for when we don't know if a name is a nickname or a hostname - special case
    Nick(String), // for when we can guess it's a nick and not a host, but have no other info
    NickHost(String, HostType),
    NickUserHost(String, String, HostType),
    Host(HostType),
}

impl FromStr for Prefix {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Prefix, Self::Err> {
        if let Some((nick, host)) = s.split_once('@') {
            if let Some((nick, user)) = nick.split_once('!') {
                if !rfc::valid_nick(nick) {
                    return Err(ParseError::InvalidNick(nick.to_string()));
                }
                if !rfc::valid_user(user) {
                    return Err(ParseError::InvalidUser(user.to_string()));
                }
                if !rfc::valid_hostname(host) {
                    return Err(ParseError::InvalidHost(host.to_string()));
                }
                Ok(Prefix::NickUserHost(nick.to_string(), user.to_string(), HostType::HostName(host.to_string())))
            } else {
                if !rfc::valid_nick(nick) {
                    return Err(ParseError::InvalidNick(nick.to_string()));
                }
                if !rfc::valid_hostname(host) {
                    return Err(ParseError::InvalidHost(host.to_string()));
                }
                Ok(Prefix::NickHost(nick.to_string(), HostType::HostName(host.to_string())))
            }
        } else {
            if rfc::valid_hostname(s) {
                Ok(Prefix::Name(s.to_string()))
            } else {
                if !valid_nick(s) {
                    return Err(ParseError::InvalidNickOrHost(s.to_string()));
                }
                Ok(Prefix::Nick(s.to_string()))
            }
        }
    }
}

pub enum Command {
    None,
}

impl FromStr for Command {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Command, Self::Err> {
        Ok(Command::None)
    }
}

pub struct Message {
    pub tags: Option<Tags>,
    pub prefix: Option<Prefix>,
    pub command: Command,
    pub parameters: Vec<String>,
}

impl Message {
    pub fn new(
        tags: Option<HashMap<String, Option<String>>>,
        prefix: Option<Prefix>,
        command: Command,
        parameters: Vec<String>,
    ) -> Result<Message, ParseError> {
        Ok(Message {
            tags,
            prefix,
            command,
            parameters,
        })
    }
}

fn take_token (iter: &mut Peekable<Chars<'_>>) -> String {
    let token = iter.take_while(|c| *c != ' ').collect::<String>();
    while let Some(c) = iter.peek() {
        if *c == ' ' {
            iter.next();
        } else {
            break;
        }
    }
    token
}

fn take_token_with_prefix (
    iter: &mut Peekable<Chars<'_>>,
    prefix_char: char
) -> Option<String> {
    match iter.peek() {
        Some(c) if *c != prefix_char => None,
        Some(_) => {
            iter.next()?;
            Some(take_token(iter))
        },
        None => None,
    }
}

fn parse_parameters (iter: &mut Peekable<Chars<'_>>) -> Vec<String> {
    let mut parameters = Vec::new();
    while let Some(c) = iter.peek() {
        if *c == ':' {
            iter.next();
            parameters.push(iter.collect::<String>());
            return parameters;
        }
        parameters.push(take_token(iter));
    }
    parameters
}

fn parse_tags (tag_string: &str) -> Result<Tags, ParseError> {
    let mut tags = HashMap::new();
    tag_string.split(';').for_each(|s| {
        if let Some((key, val)) = s.split_once('=') {
            tags.insert(key.to_string(), Some(val.to_string()));
        } else {
            tags.insert (s.to_string(), None);
        }
    });
    Ok(tags)
}

impl FromStr for Message {
    type Err = ParseError;

    fn from_str (s: &str) -> Result<Message, Self::Err> {
        let mut string_iter = s.chars().peekable();
        let tags = if let Some(t) = take_token_with_prefix(&mut string_iter, '@') {
            Some(parse_tags(&t)?)
        } else {
            None
        };
        let prefix = if let Some(p) = take_token_with_prefix(&mut string_iter, ':') {
            Some(p.parse::<Prefix>()?)
        } else {
            None
        };
        let command = take_token(&mut string_iter).parse::<Command>()?;
        let parameters = parse_parameters(&mut string_iter);

        Ok(Message {
            tags,
            prefix,
            command,
            parameters,
        })
    }
}

pub fn main() {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_token_single_space() -> Result<(), ParseError> {
        let string = "foo bar baz";
        let mut iter = string.chars().peekable();
        let token = take_token(&mut iter);
        let rest = iter.collect::<String>();
        assert!(
            token.eq("foo"),
            "`foo` should be the first token from `foo bar baz`, instead got {}", token
        );
        assert!(
            rest.eq("bar baz"),
            "`bar baz` should be the remainder from `foo bar baz`, instead got {}", rest
        );
        Ok(())
    }

    #[test]
    fn test_take_token_double_space() -> Result<(), ParseError> {
        let string = "foo  bar  baz";
        let mut iter = string.chars().peekable();
        let token = take_token(&mut iter);
        let rest = iter.collect::<String>();
        assert!(
            token.eq("foo"),
            "`foo` should be the first token from `foo bar baz`, instead got {}", token
        );
        assert!(
            rest.eq("bar  baz"),
            "`bar baz` should be the remainder from `foo bar baz`, instead got {}", rest
        );
        Ok(())
    }

    #[test]
    fn test_parse_parameters() -> Result<(), ParseError> {
        let mut iter = "foo".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 1,
            "parameter string `foo` should have len 1, got {}", len
        );
        let mut iter = "foo bar baz".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 3,
            "parameter string `foo bar baz` should have len 3, got {}", len
        );
        let mut iter = "foo :bar baz".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 2,
            "parameter string `foo :bar baz` should have len 2, got {}", len
        );
        let mut iter = "foo :".chars().peekable();
        let params = parse_parameters(&mut iter);
        assert!(
            params.len() == 2,
            "parameter string `foo :` should have len 2, got {}", params.len()
        );

        Ok(())
    }

    #[test]
    fn test_parse_tags() -> Result<(), ParseError> {
        let tags = parse_tags("id=124;foo")?;
        assert!(
            tags.contains_key("foo"),
            "`id=124;foo` tagstring must contain key foo"
        );
        Ok(())
    }

    #[test]
    fn test_parse_message() -> Result<(), ParseError> {
        let message_str = ":nickname cmd lol :stuff and things";
        let message = message_str.parse::<Message>()?;
        match message.prefix {
            Some(Prefix::Name(s)) => {
                assert!(
                    s.eq("nickname"),
                    "nick prefix with nickname `nickname` is expected, got {}", s
                );
            },
            Some(p) => {
                panic!("expected Prefix::Nick, got {:#?}", p);
            },
            None => panic!("expected Prefix::Nick, got nothing"),
        }
        Ok(())
    }
}
