use crate::irc::rfc_defs as rfc;
use std::cmp::PartialEq;

#[derive(Debug)]
pub struct Prefix {
    pub nick: Option<String>,
    pub user: Option<String>,
    pub host: Option<String>,
}

impl PartialEq for Prefix {
    fn eq(&self, other: &Self) -> bool {
        !(self.nick.as_deref() != other.nick.as_deref()
            || self.user.as_deref() != self.user.as_deref()
            || self.host.as_deref() != self.host.as_deref())
    }
}


/* here we assume Prefix contains valid nick/user/host strings */
pub fn assemble_prefix(p: Prefix) -> String {
    let mut out = String::new();
    if let Some(nick) = p.nick {
        out.push_str(&nick);
        if p.host != None {
            if let Some(user) = p.user {
                out.push_str("!");
                out.push_str(&user);
            }
            out.push_str("@");
        }
    }
    if let Some(host) = p.host {
        out.push_str(&host);
    }
    out
}

/* here we ensure only valid nick/user/host strings are parsed */
pub fn parse_prefix(s: &str) -> Option<Prefix> {
    if let Some((nick, host)) = s.split_once('@') {
        if let Some((nick, user)) = nick.split_once('!') {
            if !rfc::valid_nick(nick) {
                return None;
            }
            if !rfc::valid_user(user) {
                return None;
            }
            if !rfc::valid_host(host) {
                return None;
            }
            Some(Prefix{
                nick: Some(nick.to_string()),
                user: Some(user.to_string()),
                host: Some(host.to_string()),
            })
        } else {
            if !rfc::valid_nick(nick) {
                return None;
            }
            if !rfc::valid_host(host) {
                return None;
            }
            Some(Prefix{
                nick: Some(nick.to_string()),
                user: None,
                host: Some(host.to_string()),
            })
        }
    } else {
        if rfc::valid_host(s) {
            Some(Prefix{
                nick: None,
                user: None,
                host: Some(s.to_string()),
            })
        } else {
            if !rfc::valid_nick(s) {
                return None;
            }
            Some(Prefix{
                nick: Some(s.to_string()),
                user: None,
                host: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_prefix() {
        assert_eq!(
            assemble_prefix(Prefix {
                nick: Some("aoei".to_string()),
                user: Some("~ykstort".to_string()),
                host: Some("localhost".to_string()),
            }),
            "aoei!~ykstort@localhost",
            "format is <nick>!<user>@<host>"
        );
        assert_eq!(
            assemble_prefix(Prefix {
                nick: Some("aoei".to_string()),
                user: None,
                host: Some("localhost".to_string()),
            }),
            "aoei@localhost",
            "format is <nick>@<host>"
        );
        assert_eq!(
            assemble_prefix(Prefix {
                nick: Some("aoei".to_string()),
                user: None,
                host: None,
            }),
            "aoei",
            "format is <nick>"
        );
        assert_eq!(
            assemble_prefix(Prefix {
                nick: None,
                user: None,
                host: Some("localhost".to_string()),
            }),
            "localhost",
            "format is <host>"
        );
        assert_eq!(
            assemble_prefix(Prefix {
                nick: None,
                user: Some("~ykstort".to_string()),
                host: Some("localhost".to_string()),
            }),
            "localhost",
            "format is <host>, user/ident should not be present unless both nick and host are also present"
        );
        assert_eq!(
            assemble_prefix(Prefix {
                nick: Some("aoei".to_string()),
                user: Some("~ykstort".to_string()),
                host: None,
            }),
            "aoei",
            "format is <nick>, user/ident should not be present unless both nick and host are also present"
        );
    }

    #[test]
    fn test_parse_prefix() {
        if let Some(p) = parse_prefix("aoei!~ykstort@localhost") {
            assert_eq!(
                p,
                Prefix {
                    nick: Some("aoei".to_string()),
                    user: Some("~ykstort".to_string()),
                    host: Some("localhost".to_string()),
                },
                "nick!user@host parses correctly"
            );
        } else {
            panic!("nick!user@host parsed as invalid");
        }
        if let Some(p) = parse_prefix("aoei@localhost") {
            assert_eq!(
                p,
                Prefix {
                    nick: Some("aoei".to_string()),
                    user: None,
                    host: Some("localhost".to_string()),
                },
                "nick@host parses correctly"
            );
        } else {
            panic!("nick@host parsed as invalid");
        }
        if let Some(p) = parse_prefix("aoei") {
            assert_eq!(
                p,
                Prefix {
                    nick: None,
                    user: None,
                    host: Some("aoei".to_string()),
                },
                "nick parses as host if it can be a valid host"
            );
        } else {
            panic!("nick parsed as invalid");
        }
        if let Some(p) = parse_prefix("aoei[]") {
            assert_eq!(
                p,
                Prefix {
                    nick: Some("aoei[]".to_string()),
                    user: None,
                    host: None,
                },
                "nick like `aoei[]` parses as nick if it cannot be a valid host"
            );
        } else {
            panic!("nick parsed as invalid");
        }
        if let Some(_) = parse_prefix("aoei[]!~ykstort") {
            panic!("nick!user should be invalid parses as nick if it cannot be a valid host");
        }
    }
}