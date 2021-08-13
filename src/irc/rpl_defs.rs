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
pub const RPL_WELCOME:      u16 = 001;
pub const RPL_YOURHOST:     u16 = 002;
pub const RPL_CREATED:      u16 = 003;
pub const RPL_MYINFO:       u16 = 004;
pub const RPL_ISUPPORT:     u16 = 005;
pub const RPL_LISTSTART:    u16 = 321;
pub const RPL_LIST:         u16 = 322;
pub const RPL_LISTEND:      u16 = 323;
pub const RPL_NOTOPIC:      u16 = 331;
pub const RPL_TOPIC:        u16 = 332;
pub const RPL_TOPICWHOTIME: u16 = 333;
pub const RPL_NAMREPLY:     u16 = 353;    
pub const RPL_ENDOFNAMES:   u16 = 366;