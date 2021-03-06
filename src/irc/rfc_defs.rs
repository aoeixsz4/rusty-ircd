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
pub const MAX_MSG_SIZE: usize = 512;
pub const MAX_MSG_PARAMS: usize = 15; // including tailing, but not including COMMAND
pub const LETTER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
pub const SPECIAL: &str = "[]\\`_^{|}";
pub const DIGIT: &str = "0123456789";
pub const HEXDIGIT: &str = "0123456789ABCDEF";

// user can have any character which is not in the set CONTROL, or an '@'
pub const CONTROL: &str = "\0\r\n :";
pub const NOT_USER: &str = "\0\r\n @";
pub const NOT_CHANSTRING: &str = "\0\r\n\x07, :";

// this can probably be generalised a bit
fn matches_allowed(msg: &str, allowed: &str) -> bool {
    for item in msg.chars() {
        if !allowed.contains(item) {
            return false;
        }
    }
    true
}

fn matches_disallowed(msg: &str, disallowed: &str) -> bool {
    for item in msg.chars() {
        if disallowed.contains(item) {
            return true;
        }
    }
    false
}

/* dunno if we really need our own code for this...
 * surely there's some library shit for it...
 * according to the rfc, we should have:
 * aug BNF ipv4addr = 1*3(DIGIT) 3("." (1*3(DIGIT))
 * YES! there is, for the IRC proto! will work on that soon
 */
pub fn valid_ipv4_addr(host_addr: &str) -> bool {
    let toks: Vec<&str> = host_addr.split('.').collect();
    if toks.len() == 4 {
        // tokenizing 127...0 would give us empty string slices
        // and we would consider that invalid
        for item in toks.iter() {
            if item.is_empty() || item.len() > 3 || !matches_allowed(item, DIGIT) {
                return false;
            }
        }
        true
    } else {
        false
    }
}

// again, might be a library function for this?
// also, this only checks if the string format is generally valid
// to what the rfc 2812 says it should be,
// so for example the ipv4 parts can be 352.437.999.325,
// and we won't complain
pub fn valid_ipv6_addr(host_addr: &str) -> bool {
    let toks: Vec<&str> = host_addr.split(':').collect();
    // ipv6 should have 8 tokens
    if toks.len() == 8 {
        for item in toks.iter() {
            // no empty tokens please...
            if item.is_empty() || !matches_allowed(item, HEXDIGIT) {
                return false;
            }
        }
        true
    } else if toks.len() == 7 {
        for (i, item) in toks.iter().enumerate() {
            if item.is_empty() {
                return false;
            } else if i < 5 && &item[..] != "0" {
                return false;
            } else if i == 5 && !(&item[..] == "0" || &item[..] == "FFFF") {
                return false;
            } else if i == 6 && !valid_ipv4_addr(item) {
                return false;
            }
        }
        true
    } else {
        false
    }
}

// valid hostname/shortname
// hostname can have periods which separate shortnames
// aug BNF = shortname *( "." shortname )
pub fn valid_hostname(hostname: &str) -> bool {
    // rfc has an additional requirement that a hostname is max 63 chars
    if hostname.is_empty() || hostname.len() > 63 {
        return false;
    }

    // hostname can be tokenised by splitting with periods,
    // and each string enclosed within should be a valid 'shortname'
    let toks: Vec<&str> = hostname.split('.').collect();
    for item in toks.iter() {
        if item.is_empty() || !valid_shortname(item) {
            return false;
        }
    }

    // we did OK!!
    true
}

// aug BNF shortname = (letter/digit) *(letter/digit/"-") *(letter/digit)
// my interpretation of this is that the final *(letter/digit) is redundant
// but i think maybe it's supposed to mean "-" shouldn't be at the end OR start
pub fn valid_shortname(shortname: &str) -> bool {
    // exception if first or last letter is "-"
    if shortname.is_empty() {
        return false;
    }
    if &shortname[..1] != "-" && &shortname[..shortname.len() - 1] != "-" {
        let mut allowed = String::new();
        allowed.push_str(LETTER);
        allowed.push_str(DIGIT);
        allowed.push_str("-");
        matches_allowed(shortname, &allowed)
    } else {
        false
    }
}

// first length check might be redundant, we shouldn't really be given zero-length slices
// RFC defition: at least one a-zA-Z letter, OR 3 digits
pub fn valid_command(cmd_string: &str) -> bool {
    if cmd_string.is_empty() && matches_allowed(cmd_string, LETTER) {
        true
    } else {
        cmd_string.len() == 3 && matches_allowed(cmd_string, DIGIT)
    }
}

// this one is very permissive, according to the rfc
// user can contain any character except NUL, CR, LF, ' ', or @
pub fn valid_user(username: &str) -> bool {
    // just in case...
    if !username.is_empty() {
        !matches_disallowed(username, NOT_USER)
    } else {
        false
    }
}

// this has a way more complicated definition in the rfc,
// than what seems to be the standard '#channame' with a hash,
// followed by an a-z string of some sort, that I've always seen
// but hey ho, lets try and define it the rfc way
pub fn valid_channel(channame: &str) -> bool {
    // a channel name can be split into two chanstrings with exactly one ':'
    // but otherwise chanstrings cannot contain ':' but are otherwise
    // quite permissive
    if channame.len() < 2 {
        return false;
    }
    let (first_char, mut rest) = (channame.as_bytes()[0] as char, &channame[1..]);
    match first_char {
        '&' | '+' | '#' => (),
        '!' if rest.len() > 5 => {
            if !valid_channelid(&rest[..5]) {
                return false;
            }
            rest = &rest[5..]; // in this case maybe easier to modify the rest slice
        }
        // any other first char is WRONG
        _ => return false,
    }

    // still need to check the chanstrings...
    for item in rest.splitn(2, ':') {
        if item.is_empty() || !valid_chanstring(item) {
            return false;
        }
    }

    // if we made it this far, all checks cleared!
    true
}

// rfc says this should be a 5-character string containing A-Z or digits
pub fn valid_channelid(channelid: &str) -> bool {
    if channelid.len() == 5 {
        let mut allowed = String::new();
        allowed.push_str(UPPER);
        allowed.push_str(DIGIT);
        matches_allowed(channelid, &allowed)
    } else {
        false
    }
}

// very permissive, can be anything except NUL, BELL, CR, LF, ",", " ", ":"
pub fn valid_chanstring(chanstring: &str) -> bool {
    !matches_disallowed(chanstring, NOT_CHANSTRING)
}

// rfc states nick should be max 9 in length,
// pretty sure I've seen far longer nicks on most IRC servers though
pub fn valid_nick(nick: &str) -> bool {
    if nick.len() > 9 || nick.is_empty() {
        return false;
    }

    let mut allowed = String::new();
    allowed.push_str(LETTER);
    allowed.push_str(SPECIAL);
    // nick has different rules for the first char
    if !matches_allowed(&nick[..1], &allowed) {
        return false;
    }

    if nick.len() == 1 {
        // nothing else to check
        return true;
    }

    // push_str the rest of the options
    allowed.push_str(DIGIT);
    allowed.push_str("-");
    matches_allowed(&nick[1..], &allowed)
}
