use std::collections::btree_map::Iter;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::str::Split;

use crate::irc;
use crate::irc::err_defs as err;
use crate::irc::rfc_defs as rfc;

//use super::prefix;

pub type Tags = HashMap<String, String>;

type Prefix<T> = Vec<T>;

macro_rules! peek {
    ($token:expr) => {
        $token.clone().chars().next().unwrap()
    };
}
macro_rules! has_next {
    ($iter:expr) => {
        $iter.clone().next().is_some()
    };
}
macro_rules! peek_inner {
    ($toks:expr) => {
        $toks.clone().next().unwrap().chars().next().unwrap()
    };
}

macro_rules! length_rest {
    ($toks:expr) => {
        $toks.clone().flat_map(|tok| tok.as_bytes()).count()
    };
}

macro_rules! throw {
    () => {
        return Err(err::Error::ParseError)
    };
}

/*fn recurse(mut i: Iter<String, String>, s: &mut String) {
    match i.next() {
        None => (),
        Some((k, v)) => {
            s.push(';');
            s.push_str(k);
            if !v.is_empty() {
                s.push('=');
                //s.push_str(&copy_and_escape_value(v));
            }
        }
    }
}

pub fn assemble_tags(tags: &Tags) -> String {
    let mut out = String::new();
    let mut iter = tags.iter();
    recurse(iter, &mut out);
    out
}*/

fn split_once_infallible(s: &str, delim: char) -> (String, String) {
    match s.split_once(delim) {
        Some((l, r)) => (l.into(), r.into()),
        None => (s.into(), "".into()),
    }
}

fn parse_tags(ts: &str) -> Result<Tags, err::Error> {
    if ts.len() > rfc::MAX_TAGS_SIZE {
        throw!()
    }
    Ok(HashMap::from_iter(ts.split(';').filter_map(|s| {
        let (k, v) = split_once_infallible(s, '=');
        if rfc::valid_key(&k) && rfc::valid_value(&v) {
            Some((k, v))
        } else {
            None
        }
    })))
}

pub fn validate_prefix(p: &[&str]) -> bool {
    match p.len() {
        1 => rfc::valid_host(p[0]) || rfc::valid_nick(p[0]),
        2 => rfc::valid_nick(p[0]) && rfc::valid_host(p[1]),
        3 => rfc::valid_nick(p[0]) && rfc::valid_user(p[1]) && rfc::valid_host(p[2]),
        _ => false,
    }
}

/* here we ensure only valid nick/user/host strings are parsed */
pub fn parse_prefix(s: &str) -> Result<Prefix<&str>, err::Error> {
    let mut prefix = Vec::new();
    if let Some((nick, host)) = s.split_once('@') {
        prefix.push(nick);
        prefix.push(host);
        if let Some((nick, user)) = nick.split_once('!') {
            prefix[0] = nick;
            prefix.insert(1, user);
        }
    } else {
        prefix.push(s);
    }
    println!("prefix: {:?}", prefix);
    if !validate_prefix(&prefix) {
        throw!();
    }
    Ok(prefix)
}

pub fn parse_message(
    s: &str,
) -> Result<
    (
        Option<HashMap<String, String>>,
        Option<Prefix<&str>>,
        &str,
        Vec<&str>,
    ),
    err::Error,
> {
    let mut first_char = peek!(s);
    let mut tokens = s.split(' ');
    let tags = if first_char == '@' {
        let tag_string = tokens.next().unwrap();
        first_char = peek_inner!(tokens);
        println!("tag_string {}", tag_string);
        Some(parse_tags(tag_string.strip_prefix('@').unwrap())?)
    } else {
        None
    };
    let len_rest = length_rest!(tokens);
    if len_rest > rfc::MAX_MSG_SIZE {
        throw!();
    }
    let prefix = if first_char == ':' {
        let prefix_string = tokens.next().unwrap();
        println!("prefix_string {}", prefix_string.strip_prefix(':').unwrap());
        Some(parse_prefix(prefix_string)?)
    } else {
        None
    };
    let command = if let Some(cmd) = tokens.next() {
        cmd
    } else {
        throw!()
    };
    let mut enumerated = tokens.enumerate();
    let mut params: Vec<&str> = enumerated
        .take_while(|(i, item)| peek!(item) != ':' && *i < rfc::MAX_MSG_PARAMS)
        .map(|(_, t)| t)
        .collect::<Vec<&str>>();
    Ok((tags, prefix, command, params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let msg = "@foo=bar :aoei!~ykkie@excession NICK joanna\r\n";
        assert_eq!(parse_message(msg), Ok(()));
        let msg = ":aoei!xyz@excession LOL";
        assert_eq!(parse_message(msg), Ok(()));

        assert_eq!(
            parse_message("@l!ol=asdf;foo=bar;wut :aoei!xy!z@excession LOL"),
            Err(err::Error::ParseError)
        );
    }

    #[test]
    fn test_parse_prefix() {
        assert_eq!(
            parse_prefix("aoei!~ykstort@localhost"),
            Ok(vec!["aoei".into(), "~ykstort".into(), "localhost".into()]),
            "nick!user@host parses correctly"
        );
        assert_eq!(
            parse_prefix("aoei@localhost"),
            Ok(vec!["aoei".into(), "localhost".into()]),
            "nick@host parses correctly"
        );
        assert_eq!(
            parse_prefix("aoei"),
            Ok(vec!["aoei".into()]),
            "nick parses correctly"
        );
        assert_eq!(
            parse_prefix("aoei[]"),
            Ok(vec!["aoei[]".into()]),
            "nick like `aoei[]` parses as nick if it cannot be a valid host"
        );
        assert_eq!(
            parse_prefix("aoei[]!~ykstort"),
            Err(err::Error::ParseError),
            "nick like `aoei[]` parses as nick if it cannot be a valid host"
        );
    }
    #[test]
    fn test_empty_or_missing_value() {
        let tags = parse_tags("foo=").unwrap();
        let foo = tags.get("foo").unwrap();
        assert_eq!(foo.len(), 0, "key with empty val contains empty string");
        let tags = parse_tags("foo").unwrap();
        let foo = tags.get("foo").unwrap();
        assert_eq!(foo.len(), 0, "key with no val contains empty string");
    }

    #[test]
    fn test_invalid_key_ignored_silently() {
        let tags = parse_tags("foo=bar;x'=jan;bo=jack").unwrap();
        assert_eq!(
            tags.contains_key("x'"),
            false,
            "invalid key `x'` shouldn't be saved"
        );
        assert_eq!(tags.get("foo").unwrap(), "bar", "`foo` key still processed");
        assert_eq!(tags.get("bo").unwrap(), "jack", "`bo` key still processed");
    }

    #[test]
    fn test_client_tag_prefix_ok() {
        let tags = parse_tags("+foo=bar").unwrap();
        assert_eq!(
            tags.contains_key("+foo"),
            true,
            "client key `+foo` should be saved"
        );
        assert_eq!(tags.get("+foo").unwrap(), "bar", "`+foo` key processed");
    }

    #[test]
    fn test_unescape_special() {
        let tags = parse_tags("foo=ab\\r\\ncd").unwrap();
        assert_eq!(
            tags.get("foo").unwrap(),
            "ab\r\ncd",
            "escapes in value of foo should be translated to literal CR LF"
        );
        let tags = parse_tags("foo=ab\\s\\:cd").unwrap();
        assert_eq!(
            tags.get("foo").unwrap(),
            "ab ;cd",
            "escapes in value of foo should be translated to literal space and semicolon"
        );
        let tags = parse_tags("foo=ab\\s\\").unwrap();
        assert_eq!(
            tags.get("foo").unwrap(),
            "ab ",
            "trailing `\\` should be removed"
        );
        let tags = parse_tags("foo=ab\\s\\b").unwrap();
        assert_eq!(
            tags.get("foo").unwrap(),
            "ab b",
            "invalid escape just removes `\\`"
        );
    }

    #[test]
    fn test_last_key_supersedes() {
        let tags = parse_tags("foo=bar;foo=baz").unwrap();
        assert_eq!(tags.contains_key("foo"), true, "foo key is saved (twice)");
        assert_eq!(
            tags.get("foo").unwrap(),
            "baz",
            "`foo` key contains the last value in the tag string"
        );
    }

    #[test]
    fn test_assembly() {
        let tags = parse_tags("foo=bar;asdf=baz").unwrap();
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo=bar;asdf=baz" || assembled == "asdf=baz;foo=bar",
            true,
            "string `foo=bar;asdf=baz` is reproduced (possibly in another order)"
        );
    }

    #[test]
    fn test_assembly_empty_key() {
        let tags = parse_tags("foo=;asdf=baz").unwrap();
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo;asdf=baz" || assembled == "asdf=baz;foo",
            true,
            "string `foo=;asdf=baz` is reproduced (in some order) with foo's `=` dropped"
        );
    }

    #[test]
    fn test_assembly_escape() {
        let tags = parse_tags("foo=;asdf=baz\\n").unwrap();
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo;asdf=baz\\n" || assembled == "asdf=baz\\n;foo",
            true,
            "string `foo=;asdf=baz\\n` is reproduced (in some order) with escaped \\n, got {}",
            assembled
        );
    }
}
