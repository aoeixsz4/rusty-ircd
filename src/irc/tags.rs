use core::iter::Iterator;
use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::str::Chars;
use crate::irc::rfc_defs as rfc;

pub type Tags = HashMap<String, String>;

struct Escaper<'a> {
    inner_iter: Chars<'a>,
    insert: Option<char>,
}

impl Iterator for Escaper<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(saved) = self.insert {
            self.insert = None;
            return Some(saved);
        }
        if let Some(next) = self.inner_iter.next() {
            match next {
                ' ' => self.insert = Some('s'),
                '\r' => self.insert = Some('r'),
                '\n' => self.insert = Some('n'),
                ';' => self.insert = Some(':'),
                '\\' => self.insert = Some('\\'),
                _ => return Some(next),
            }
            return Some('\\');
        }
        None
    }
}

impl<'a> Escaper<'a> {
    fn from_str(s: &'a str) -> Self {
        Escaper {
            inner_iter: s.chars(),
            insert: None,
        }
    }
}

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

fn copy_and_escape_value (value: &str) -> String {
    let iter = Escaper::from_str(value);
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

fn recurse (mut i: Iter<String, String>, s: &mut String) {
    match i.next() {
        None => (),
        Some((k, v)) => {
            s.push_str(";");
            s.push_str(k);
            if v.len() > 0 {
                s.push_str("=");
                s.push_str(&copy_and_escape_value(v));
            }
        },
    }
}

pub fn assemble_tags (tags: &Tags) -> String {
    let mut out = String::new();
    let mut iter = tags.iter();
    if let Some((k, v)) = iter.next() {
        out.push_str(k);
        if v.len() > 0 {
            out.push_str("=");
            out.push_str(&copy_and_escape_value(v));
        }
    }
    recurse(iter, &mut out);
    out
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
 
    #[test]
    fn test_assembly() {
        let tags = parse_tags("foo=bar;asdf=baz");
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo=bar;asdf=baz"
            || assembled == "asdf=baz;foo=bar", true,
            "string `foo=bar;asdf=baz` is reproduced (possibly in another order)"
        );
    }
 
    #[test]
    fn test_assembly_empty_key() {
        let tags = parse_tags("foo=;asdf=baz");
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo;asdf=baz"
            || assembled == "asdf=baz;foo", true,
            "string `foo=;asdf=baz` is reproduced (in some order) with foo's `=` dropped"
        );
    }
 
    #[test]
    fn test_assembly_escape() {
        let tags = parse_tags("foo=;asdf=baz\\n");
        let assembled = assemble_tags(&tags);
        assert_eq!(
            assembled == "foo;asdf=baz\\n"
            || assembled == "asdf=baz\\n;foo", true,
            "string `foo=;asdf=baz\\n` is reproduced (in some order) with escaped \\n, got {}", assembled
        );
    }
}