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
use crate::irc::err_defs as err;
use crate::irc::tags::{Tags, assemble_tags, parse_tags};
use crate::irc::prefix::{Prefix, assemble_prefix, parse_prefix};
use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::{Chars, FromStr};

#[derive(Debug)]
pub struct Message {
    pub tags: Option<Tags>,
    pub prefix: Option<Prefix>,
    pub command: String,
    pub parameters: Vec<String>,
}

impl Message {
    pub fn new(
        tags: Option<HashMap<String, String>>,
        prefix: Option<Prefix>,
        command: String,
        parameters: Vec<String>,
    ) -> Message {
        Message {
            tags,
            prefix,
            command,
            parameters,
        }
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

fn take_token_with_prefix<'a> (s: &'a str, prefix: &str) -> (&'a str, &'a str) {
    let (p_len, s_len) = (prefix.len(), s.len());
    if p_len < s_len && &s[..p_len] == prefix {
        if let Some(i) = s.find(" ") {
            return (&s[p_len..i], &s[i+1..]);
        }
    }
    ("", s)
}

fn assemble_parameters (params: &Vec<String>) -> String {
    let mut out = String::new();
    for i in 0 .. params.len() {
        if i != 0 {
            out.push_str(" ");
        }
        if i == params.len() - 1 && params[i].find(' ') != None {
            out.push_str(":");
        }
        out.push_str(&params[i]);
    }
    out
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

impl FromStr for Message {
    type Err = err::Error;

    fn from_str (s: &str) -> Result<Message, Self::Err> {
        let (tag_string, s) = take_token_with_prefix(s, "@");
        let tags = if tag_string.is_empty() {
            if tag_string.as_bytes().len() + 2 > rfc::MAX_TAGS_SIZE_TOTAL {
                return Err(err::Error::ParseError);
            }
            Some(parse_tags(&tag_string))
        } else {
            None
        };
        let rest = string_iter.collect::<String>();
        if rest.as_bytes().len() > rfc::MAX_MSG_SIZE {
            return Err(err::Error::ParseError);
        }
        string_iter = rest.chars().peekable();
        let prefix = if let Some(p) = take_token_with_prefix(&mut string_iter, ':') {
            parse_prefix(&p)
        } else {
            None
        };
        let command = take_token(&mut string_iter);
        if !rfc::valid_command(&command) {
            return Err(err::Error::ParseError);
        }
        let parameters = parse_parameters(&mut string_iter);

        Ok(Message {
            tags,
            prefix,
            command,
            parameters,
        })
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let tags = if let Some(t) = &self.tags {
            format!("@{} ", assemble_tags(t))
        } else {
            "".to_string()
        };
        /* need to do something if tag_string.as_bytes().len() exceeds rfc::MAX_TAGS_SIZE_TOTAL */
        if tags.as_bytes().len() > rfc::MAX_TAGS_SIZE_TOTAL {
            return Err(fmt::Error);
        }
        let prefix = if let Some(p) = &self.prefix {
            format!(":{} ", assemble_prefix(p))
        } else {
            "".to_string()
        };
        let message = if self.parameters.len() > 0 {
            format!("{}{} {}\r\n", prefix, self.command, &assemble_parameters(&self.parameters))
        } else {
            format!("{}{}\r\n", prefix, self.command)
        };
        if message.as_bytes().len() > rfc::MAX_MSG_SIZE {
            return Err(fmt::Error);
        }
        write!(f, "{}{}", tags, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take_token_single_space() {
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
    }

    #[test]
    fn test_take_token_double_space() {
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
    }

    #[test]
    fn test_parse_parameters_nospaces() {
        let mut iter = "foo".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 1,
            "parameter string `foo` should have len 1, got {}", len
        );
    }

    #[test]
    fn test_parse_parameters_single_spaced() {
        let mut iter = "foo bar baz".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 3,
            "parameter string `foo bar baz` should have len 3, got {}", len
        );
    }

    #[test]
    fn test_parse_parameters_with_colon() {
        let mut iter = "foo :bar baz".chars().peekable();
        let len = parse_parameters(&mut iter).len();
        assert!(
            len == 2,
            "parameter string `foo :bar baz` should have len 2, got {}", len
        );
    }

    #[test]
    fn test_parse_parameters_with_colon_empty() {
        let mut iter = "foo :".chars().peekable();
        let params = parse_parameters(&mut iter);
        assert!(
            params.len() == 2,
            "parameter string `foo :` should have len 2, got {}", params.len()
        );
    }

    #[test]
    fn test_parse_message_valid() {
        let message_str = ":nickname cmd lol :stuff and things";
        if let Ok(message) = message_str.parse::<Message>() {
            match message.prefix.unwrap().host {
                Some(s) => {
                    assert!(
                        true,
                        "nick prefix with nickname `nickname` is expected, got {}", s
                    );
                },
                None => panic!("expected Prefix::Nick, got nothing"),
            }
        }
    }
    
    #[test]
    fn test_parse_message_tags_twice() {
        let message_str = "@tag @doubletag :stuff and things";
        match message_str.parse::<Message>() {
            Ok(_) => panic!("expected invalid command error for {}, got ok", message_str),
            Err(err::Error::ParseError) => (),
            Err(_) => panic!("expected invalid command error"),
        }
    }
    
    #[test]
    fn test_parse_message_prefix_twice() {
        let message_str = ":foobar!x@y :stuff and things";
        match message_str.parse::<Message>() {
            Ok(_) => panic!("expected invalid command error for {}, got ok", message_str),
            Err(err::Error::ParseError) => (),
            Err(_) => panic!("expected invalid command error"),
        }
    }
    
    #[test]
    fn test_parse_message_tags_after_prefix() {
        let message_str = ":foobar!x@y @tags and things";
        match message_str.parse::<Message>() {
            Ok(_) => panic!("expected invalid command error for {}, got ok", message_str),
            Err(err::Error::ParseError) => (),
            Err(_) => panic!("expected invalid command error but got"),
        }
    }

    #[test]
    fn test_message_too_long() {
        let message_str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        match message_str.parse::<Message>() {
            Ok(_) => panic!("expected message too long error for"),
            Err(err::Error::ParseError) => (),
            Err(_) => panic!("expected message too long error"),
        }
    }

    #[test]
    fn test_display_message() {
        let mut tags = HashMap::new();
        tags.insert("foo".to_string(), "bar".to_string());
        let prefix = Prefix { nick: Some("aoei".to_string()), user: Some("~ykkie".to_string()), host: Some("excession".to_string()) };
        let mut my_params = Vec::new();
        my_params.push("joanna".to_string());
        let msg = Message::new(Some(tags), Some(prefix), "NICK".to_string(), my_params);
        assert_eq!(
            "@foo=bar :aoei!~ykkie@excession NICK joanna\r\n",
            format!("{}", msg),
            "forms a valid IRC protocol message with @tags :prefix COMMAND param CR LF"
        );
    }

    #[test]
    fn test_display_message_trailing_param() {
        let prefix = Prefix { nick: Some("aoei".to_string()), user: Some("~ykkie".to_string()), host: Some("excession".to_string()) };
        let mut my_params = Vec::new();
        my_params.push("this is a lengthy message with spaces in".to_string());
        let msg = Message::new(None, Some(prefix), "PRIVMSG".to_string(), my_params);
        assert_eq!(
            ":aoei!~ykkie@excession PRIVMSG :this is a lengthy message with spaces in\r\n",
            format!("{}", msg),
            "trailing parameter should be prefixed with a colon"
        );
    }

    #[test]
    fn test_display_message_no_params() {
        let prefix = Prefix { nick: Some("aoei".to_string()), user: Some("~ykkie".to_string()), host: Some("excession".to_string()) };
        let my_params = Vec::new();
        let msg = Message::new(None, Some(prefix), "LIST".to_string(), my_params);
        assert_eq!(
            ":aoei!~ykkie@excession LIST\r\n",
            format!("{}", msg),
            "trailing parameter should be prefixed with a colon"
        );
    }

    #[test]
    fn test_assemble_parameters() {
        let my_params = vec![String::from("asdf"), String::from("foo"), String::from("trailing param")];
        assert_eq!(
            assemble_parameters(&my_params),
            String::from("asdf foo :trailing param"),
            "trailling param must be prefixed with a colon"
        );
    }
}
