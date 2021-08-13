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
use crate::irc::error::Error as ircError;
use std::str::FromStr;

#[derive(Debug)]
pub enum Command {
    Nick,
    User,
    Privmsg,
    Notice,
    Join,
    Part,
    Topic,
    List,
}

impl FromStr for Command {
    type Err = ircError;

    fn from_str(s: &str) -> Result<Command, Self::Err> {
        let upper = s.to_ascii_uppercase();
        match upper.as_str() {
            "NICK" => Ok(Command::Nick),
            "USER" => Ok(Command::User),
            "PRIVMSG" => Ok(Command::Privmsg),
            "NOTICE" => Ok(Command::Notice),
            "JOIN" => Ok(Command::Join),
            "PART" => Ok(Command::Part),
            "TOPIC" => Ok(Command::Topic),
            "LIST" => Ok(Command::List),
            _ => Err(ircError::UnknownCommand(s.to_string())),
        }
    }
}
