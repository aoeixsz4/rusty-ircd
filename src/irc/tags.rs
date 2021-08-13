use core::iter::Iterator;
use std::{collections::HashMap, str::Chars};
use crate::irc::rfc_defs as rfc;

pub type Tags = HashMap<String, String>;

struct Unescaper<'a> {
    inner_iter: Chars<'a>,
}

impl Iterator for Unescaper<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner_iter.next() {
            Some (c) if c == '\\' => {
                match self.inner_iter.next() {
                    Some('\\') => Some('\\'),
                    Some(':') => Some(';'),
                    Some('r') => Some('\r'),
                    Some('s') => Some(' '),
                    Some('n') => Some('\n'),
                    Some(o) => Some(o),
                    None => None,
                }
            },
            Some (c) => Some(c),
            None => None,
        }
    }
}

impl<'a> Unescaper<'a> {
    fn from_str(s: &'a str) -> Self {
        Unescaper {
            inner_iter: s.chars()
        }
    }
}

fn copy_and_unescape_value (value: &str) -> String {
    let iter = Unescaper::from_str(value);
    iter.filter_map(|x| Some(x)).collect::<String>()
}

pub fn parse_tags (tag_string: &str) -> Tags {
    let mut tags = HashMap::new();
    for s in tag_string.split(';') {
        if let Some((key, val)) = s.split_once('=') {
            if rfc::valid_key(key) && rfc::valid_value(val) {
                tags.insert(key.to_string(), copy_and_unescape_value(val));
            }
        } else {
            if rfc::valid_key(s) {
                tags.insert (s.to_string(), "".to_string());
            }   
        }
    }
    return tags;
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_or_missing_value() {
        let tags = parse_tags("foo=");
        let foo = tags.get("foo").unwrap();
        assert_eq!(foo.len(), 0, "key with empty val contains empty string");
        let tags = parse_tags("foo");
        let foo = tags.get("foo").unwrap();
        assert_eq!(foo.len(), 0, "key with no val contains empty string");
    }

    #[test]
    fn test_invalid_key_ignored_silently() {
        let tags = parse_tags("foo=bar;x'=jan;bo=jack");
        assert_eq!(tags.contains_key("x'"), false, "invalid key `x'` shouldn't be saved");
        assert_eq!(tags.get("foo").unwrap(), "bar", "`foo` key still processed");
        assert_eq!(tags.get("bo").unwrap(), "jack", "`bo` key still processed");
    }
 
    #[test]
    fn test_client_tag_prefix_ok() {
        let tags = parse_tags("+foo=bar");
        assert_eq!(tags.contains_key("+foo"), true, "client key `+foo` should be saved");
        assert_eq!(tags.get("+foo").unwrap(), "bar", "`+foo` key processed");
    }

    #[test]
    fn test_unescape_special() {
        let tags = parse_tags("foo=ab\\r\\ncd");
        assert_eq!(tags.get("foo").unwrap(), "ab\r\ncd", "escapes in value of foo should be translated to literal CR LF");
        let tags = parse_tags("foo=ab\\s\\:cd");
        assert_eq!(tags.get("foo").unwrap(), "ab ;cd", "escapes in value of foo should be translated to literal space and semicolon");
        let tags = parse_tags("foo=ab\\s\\");
        assert_eq!(tags.get("foo").unwrap(), "ab ", "trailing `\\` should be removed");
        let tags = parse_tags("foo=ab\\s\\b");
        assert_eq!(tags.get("foo").unwrap(), "ab b", "invalid escape just removes `\\`");
    }
 
    #[test]
    fn test_last_key_supersedes() {
        let tags = parse_tags("foo=bar;foo=baz");
        assert_eq!(tags.contains_key("foo"), true, "foo key is saved (twice)");
        assert_eq!(tags.get("foo").unwrap(), "baz", "`foo` key contains the last value in the tag string");
    }
}