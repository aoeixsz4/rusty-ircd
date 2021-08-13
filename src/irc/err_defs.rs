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
pub const ERR_UNKNOWNERROR:      u16 = 400;
pub const ERR_NOSUCHNICK:        u16 = 401;
pub const ERR_NOSUCHCHANNEL:     u16 = 403;
pub const ERR_CANNOTSENDTOCHAN:  u16 = 404;
pub const ERR_UNKNOWNCOMMAND:    u16 = 421;
pub const ERR_ERRONEOUSNICKNAME: u16 = 432;
pub const ERR_NICKNAMEINUSE:     u16 = 433;
pub const ERR_NOTONCHANNEL:      u16 = 442;
pub const ERR_NOTREGISTERED:     u16 = 451;
pub const ERR_NEEDMOREPARAMS:    u16 = 461;
pub const ERR_ALREADYREGISTRED:  u16 = 462;
pub const ERR_CHANOPRIVSNEEDED:  u16 = 482;