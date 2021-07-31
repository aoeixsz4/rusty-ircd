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
use crate::irc::message::ParseError;
use crate::irc::rfc_defs as rfc;
use std::str::FromStr;

#[derive(Debug)]
pub enum Command {
    None,
}

impl FromStr for Command {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Command, Self::Err> {
        if !rfc::valid_command(s) {
            return Err(ParseError::InvalidCommand(s.to_string()));
        }
        let upper = s.to_ascii_uppercase();
        match &upper {
            "NICK" => Command::Nick,
            "USER" => Command::User,
            "PRIVMSG" => Command::Privmsg,
            "NOTICE" => Command::Notice,
            "JOIN" => Command::Join,
            "PART" => Command::Part,
            "TOPIC" => Command::Topic,
            "LIST" => Command::List,
            _ => Err(ParseError::UnknownCommand(s)),
        }
        Ok(Command::None)
    }
}

