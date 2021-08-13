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
use std::net::{Ipv4Addr, Ipv6Addr};
pub const MAX_HOSTNAME_SIZE: usize = 253;
pub const MAX_SHORTNAME_SIZE: usize = 63;
pub const MAX_CHANNAME_SIZE: usize = 50;
pub const MAX_NICKNAME_SIZE: usize = 9;
pub const CHANNELID_SIZE: usize = 5;
pub const MAX_MSG_SIZE: usize = 512;
pub const MAX_TAGS_SIZE: usize = 4094;
pub const MAX_TAGS_SIZE_TOTAL: usize = 8191;
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
pub const NOT_TAGVAL: &str = "\0\r\n ;";
pub const NOT_CHANSTRING: &str = "\0\r\n\x07, :";

// this can probably be generalised a bit
// this is really asking "is msg a subset of allowed"
fn matches_allowed(msg: &str, allowed: &str) -> bool {
    for item in msg.chars() {
        if !allowed.contains(item) {
            return false;
        }
    }
    true
}

// this is asking if msg contains any member of disallowed
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
    match host_addr.parse::<Ipv4Addr>() {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn valid_ipv6_addr(host_addr: &str) -> bool {
    match host_addr.parse::<Ipv6Addr>() {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn valid_key(mut tag_key: &str) -> bool {
    let mut allowed = String::new();
    allowed.push_str(LETTER);
    allowed.push_str(DIGIT);
    allowed.push_str("-");
    if let Some(c) = tag_key.chars().nth(0) {
        if c == '+' {
            /* luckily we know '+' is a single byte character... */
            tag_key = &tag_key[1..];
        }
    } else {
        return false;
    }
    if let Some((vendor, key)) = tag_key.split_once('/') {
        if !valid_hostname(vendor) || !matches_allowed(key, &allowed) {
            false
        } else {
            true
        }
    } else {
        if !matches_allowed(tag_key, &allowed) {
            false
        } else {
            true
        }
    }
}

pub fn valid_value(val: &str) -> bool {
    !matches_disallowed(val, NOT_TAGVAL)
}


// valid hostname/shortname
// hostname can have periods which separate shortnames
// aug BNF = shortname *( "." shortname )
pub fn valid_hostname(hostname: &str) -> bool {
    // rfc has an additional requirement that a hostname is max 63 chars
    if hostname.is_empty() || hostname.len() > MAX_HOSTNAME_SIZE {
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

pub fn valid_host(host: &str) -> bool {
    valid_ipv4_addr(host)
        || valid_ipv4_addr(host)
        || valid_hostname(host)
}

// aug BNF shortname = (letter/digit) *(letter/digit/"-") *(letter/digit)
// my interpretation of this is that the final *(letter/digit) is redundant
// but i think maybe it's supposed to mean "-" shouldn't be at the end OR start
pub fn valid_shortname(shortname: &str) -> bool {
    // exception if first or last letter is "-"
    if shortname.is_empty()
        || shortname.len() > MAX_SHORTNAME_SIZE
        || shortname.chars().nth(0).unwrap() == '-'
        || shortname.chars().last().unwrap() == '-'
    {
        return false;
    }

    let mut allowed = String::new();
    allowed.push_str(LOWER);
    allowed.push_str(DIGIT);
    allowed.push_str("-");
    matches_allowed(shortname, &allowed)
}

// first length check might be redundant, we shouldn't really be given zero-length slices
// RFC defition: at least one a-zA-Z letter, OR 3 digits
pub fn valid_command(cmd_string: &str) -> bool {
    if !cmd_string.is_empty() && matches_allowed(cmd_string, LETTER) {
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
// ! chans must have a 5-char 'channel ID' followed by a chanstring
pub fn valid_channel(channame: &str) -> bool {
    // a channel name can be split into two chanstrings with exactly one ':'
    // but otherwise chanstrings cannot contain ':' but are otherwise
    // quite permissive
    if channame.len() < 2 || channame.len() > MAX_CHANNAME_SIZE {
        return false;
    }
    let mut name_iter = channame.chars();
    let first_char = name_iter.next().unwrap();
    let mut chanstring: String = name_iter.collect();
    match first_char {
        '&' | '+' | '#' => (),
        '!' if chanstring.len() > 5 => {
            let channelid: String = chanstring.chars().take(5).collect();
            chanstring = chanstring.chars().skip(5).collect();
            if !valid_channelid(&channelid) {
                return false;
            }
        }
        // any other first char is WRONG
        _ => return false,
    }

    // still need to check the chanstrings...
    for item in chanstring.splitn(2, ':') {
        if item.is_empty() || !valid_chanstring(item) {
            return false;
        }
    }

    // if we made it this far, all checks cleared!
    true
}

// rfc says this should be a 5-character string containing A-Z or digits
pub fn valid_channelid(channelid: &str) -> bool {
    if channelid.len() != CHANNELID_SIZE {
        return false;
    }
    let mut allowed = String::new();
    allowed.push_str(UPPER);
    allowed.push_str(DIGIT);
    matches_allowed(channelid, &allowed)
}

// very permissive, can be anything except NUL, BELL, CR, LF, ",", " ", ":"
pub fn valid_chanstring(chanstring: &str) -> bool {
    !matches_disallowed(chanstring, NOT_CHANSTRING)
}

// rfc states nick should be max 9 in length,
// pretty sure I've seen far longer nicks on most IRC servers though
pub fn valid_nick(nick: &str) -> bool {
    if nick.len() > MAX_NICKNAME_SIZE || nick.is_empty() {
        return false;
    }

    let mut allowed = String::new();
    allowed.push_str(LETTER);
    allowed.push_str(SPECIAL);
    let first: String = nick.chars().take(1).collect();
    // nick has different rules for the first char
    if !matches_allowed(&first, &allowed) {
        return false;
    }

    // push_str the rest of the options
    let rest: String = nick.chars().skip(1).collect();
    if rest.is_empty() {
        return true;
    }
    allowed.push_str(DIGIT);
    allowed.push_str("-");
    matches_allowed(&rest, &allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_invert_set(char_set: &str) -> String {
        let mut inverted_set = String::new();
        for c in '\0'..(255 as char) {
            if !char_set.contains(c) {
                inverted_set.push(c);
            }
        }
        inverted_set
    }

    #[test]
    fn match_allowed_or_disallowed_sets() {
        assert_eq!(matches_allowed("abc", "abcdef"), true);
        assert_eq!(matches_allowed("abc", "abdef"), false);
        assert_eq!(matches_disallowed("abc", "adef"), true);
        assert_eq!(matches_disallowed("abc", "def"), false);
    }

    #[test]
    fn hostname_cases() {
        let mut max_short1 = String::new();
        let mut max_short2 = String::new();
        let mut max_short3 = String::new();
        let mut max_short4 = String::new();
        for _ in 0..MAX_SHORTNAME_SIZE {
            max_short1.push('a');
        }
        for _ in 0..MAX_SHORTNAME_SIZE {
            max_short2.push('b');
        }
        for _ in 0..MAX_SHORTNAME_SIZE {
            max_short3.push('c');
        }
        for _ in 0..MAX_SHORTNAME_SIZE - 2 {
            max_short4.push('d');
        }
        let max_len_hostname = format!(
            "{}.{}.{}.{}",
            max_short1, max_short2, max_short3, max_short4
        );
        max_short4.push('d');
        let over_max_len_hostname = format!(
            "{}.{}.{}.{}",
            max_short1, max_short2, max_short3, max_short4
        );
        assert!(!valid_hostname(""), "an empty string cannot be a hostname");
        assert!(
            !valid_hostname(&over_max_len_hostname),
            "hostname must be {} chars or less",
            MAX_HOSTNAME_SIZE
        );
        assert!(
            valid_hostname(&max_len_hostname),
            "{}-char hostname is valid",
            MAX_HOSTNAME_SIZE
        );
        assert!(
            !valid_hostname("foo..bar"),
            "hostname cannot contain two consecutive periods"
        );
        assert!(
            !valid_hostname(".bar"),
            "hostname cannot begin with a period"
        );
        assert!(
            !valid_hostname("foo."),
            "hostname cannot begin with a period"
        );
    }

    #[test]
    fn shortname_cases() {
        let mut max_shortname = String::new();
        for _ in 0..MAX_SHORTNAME_SIZE {
            max_shortname.push('a');
        }
        let over_max_shortname = format!("{}{}", max_shortname, "1");
        assert!(!valid_shortname("-asdf"), "shortname may not begin with -");
        assert!(!valid_shortname("asdf-"), "shortname may not end with -");
        assert!(
            valid_shortname("as-df"),
            "shortname may contain - in a non-terminal position"
        );
        assert!(
            valid_shortname("1as7-df8"),
            "shortname may contain digits in any position"
        );
        assert!(
            valid_shortname(&max_shortname),
            "shortname may contain up to {} chars",
            MAX_SHORTNAME_SIZE
        );
        assert!(
            !valid_shortname(&over_max_shortname),
            "shortname may contain no more than {} chars",
            MAX_SHORTNAME_SIZE
        );
        for invalid_char in make_invert_set(&format!("{}{}-", LOWER, DIGIT)).chars() {
            assert!(
                !valid_shortname(&format!("foo-{}bar", invalid_char)),
                "shortname cannot contain {}",
                invalid_char
            );
        }
        assert!(
            !valid_shortname("foo-🤔bar"),
            "shortname may not contain emojis such as 🤔 or other UTF-8"
        );
    }

    #[test]
    fn command_cases() {
        assert!(
            valid_command("100"),
            "3-digit number is valid command string"
        );
        assert!(
            !valid_command("1000"),
            "4-digit number is invalid command string"
        );
        assert!(
            !valid_command("1000"),
            "4-digit number is invalid command string"
        );
        assert!(
            valid_command("asdf"),
            "alphabetical is valid command"
        );
        assert!(
            !valid_command("asd12"),
            "can't mix letters and numbers"
        );
        assert!(
            valid_command("abCD"),
            "any case is fine"
        );
    }

    #[test]
    fn tagval_cases() {
        assert!(
            valid_value("foobar"),
            "a word is valid",
        );
        assert!(
            !valid_value("foo bar"),
            "space is not allowed",
        );
        assert!(
            !valid_value("foo;bar"),
            "; is not allowed",
        );
        assert!(
            valid_value("foo😁bar"),
            "emoji is allowed",
        );
    }

    #[test]
    fn user_cases() {
        assert!(
            valid_user("\\kekF00rifk{}"),
            "user string may contain all kinds of weird chars",
        );
        assert!(!valid_user(""), "user string may not be empty");
        for invalid_char in NOT_USER.chars() {
            assert!(
                !valid_user(&format!("lol{}", invalid_char)),
                "user string may not contain {}",
                invalid_char
            );
        }
        assert!(
            valid_user("foo-🤔bar"),
            "user string may contain emojis/other unicode"
        );
        for valid_char in make_invert_set(NOT_USER).chars() {
            assert!(
                valid_user(&format!("lol{}", valid_char)),
                "user string may contain {}",
                valid_char
            );
        }
    }

    #[test]
    fn chan_string_cases() {
        assert!(!valid_channel(""), "channel cannot be an empty string");
        assert!(
            !valid_channel("#"),
            "channel cannot be fewer than 2 chars long"
        );
        assert!(
            valid_channel("!123456"),
            "! chan of may have 6 ore more digits after the"
        );
        assert!(
            !valid_channel("!12345"),
            "! chans must contain both channel ID (5 chars) and chanstring"
        );
        assert!(
            valid_channel("!123ABabc"),
            "! chans contain uppercase letters in the chan ID"
        );
        assert!(
            !valid_channel("!123Ababc"),
            "! chans may not contain lowercase letters in the chan ID"
        );
        for invalid_char in make_invert_set("&+#!").chars() {
            assert!(
                !valid_channel(&format!("{}ABC12abc", invalid_char)),
                "{} may not be first char of channel name",
                invalid_char
            );
        }
        for invalid_char in make_invert_set(&format!("{}{}", UPPER, DIGIT)).chars() {
            assert!(
                !valid_channel(&format!("!{}", invalid_char)),
                "! chans may not contain {}",
                invalid_char
            );
        }
        for invalid_char in NOT_CHANSTRING.chars() {
            assert!(
                !valid_channel(&format!("#{}", invalid_char)),
                "[#+&] chans may not contain {}",
                invalid_char
            );
        }
        assert!(
            valid_channel("#foobar"),
            "#foobar is an allowed channel name"
        );
        assert!(
            valid_channel("&foo:bar"),
            "&foo:bar is an allowed channel name"
        );
        assert!(
            !valid_channel("&foo:bar:baz"),
            "channel may not contain two : separators"
        );
        assert!(
            valid_channel("!123ABfoo:bar"),
            "! channel may also contain a : separator for the chan strings"
        );
        assert!(
            !valid_channel("&foo:"),
            "channel may not contain an empty string after a : delimiter"
        );
        assert!(
            !valid_channel("#fooooooooooooooooooooooooooooooooooooooooooooooooo"),
            "channel name may not contain more than 50 chars"
        );
    }
    #[test]
    fn key_cases() {
        assert!(
            !valid_key("asd{f"),
            "key cannot contain a curly brace char",
        );
        assert!(
            valid_key("asd/f"),
            "key can contain /",
        );
        assert!(
            !valid_key("asd../f"),
            "key can contain / but must be valid host before",
        );
        assert!(
            valid_key("asdf1234-"),
            "key can be alphanumeric plus hyphen",
        );
        assert!(
            valid_key("+asdf1234-"),
            "key can be alphanumeric plus hyphen, can start with +",
        );
        assert!(
            !valid_key("asd+f1234-"),
            "key can be alphanumeric plus hyphen, cannot contain + after the first char",
        );
    }

    #[test]
    fn nick_cases() {
        assert!(
            !valid_nick("abcdefghij"),
            "max nickname lengthis {}",
            MAX_NICKNAME_SIZE
        );
        assert!(valid_nick("abcdefghi"), "a-z is allowed in nicks");
        assert!(valid_nick("abcdefghi"), "a-z is allowed in nicks");
        for invalid_char in make_invert_set(&format!("{}{}", LETTER, SPECIAL)).chars() {
            assert!(
                !valid_nick(&format!("{}ABCabc", invalid_char)),
                "{} may not be first char of nick name",
                invalid_char
            );
        }
        for valid_char in format!("{}{}", LETTER, SPECIAL).chars() {
            assert!(
                valid_nick(&format!("{}ABCabc", valid_char)),
                "{} may be first char of nick name",
                valid_char
            );
        }
        for invalid_char in make_invert_set(&format!("{}{}{}-", LETTER, SPECIAL, DIGIT)).chars() {
            assert!(
                !valid_nick(&format!("a{}abc", invalid_char)),
                "{} may not be a non-first char of nick name",
                invalid_char
            );
        }
        for valid_char in format!("{}{}{}-", LETTER, SPECIAL, DIGIT).chars() {
            assert!(
                valid_nick(&format!("a{}abc", valid_char)),
                "{} may be a non-first char of nick name",
                valid_char
            );
        }
    }
}
