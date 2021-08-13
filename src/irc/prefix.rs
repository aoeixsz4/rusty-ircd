use crate::irc::rfc_defs as rfc;

#[derive(Debug)]
pub struct Prefix {
    pub nick: Option<String>,
    pub user: Option<String>,
    pub host: Option<String>,
}

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