mod parser;
mod client;
mod buffer;
mod irc;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main () {
    let listener = TcpListener::bind("127.0.0.1:6667").unwrap();
    let stream = listener.incoming().nth(0).unwrap().unwrap();
    loop {
        let mut irc_msg_buf = [0; 512];
        
        // this gives us a String ending in LF, we want CR-LF
        //io::stdin().read_line(&mut irc_msg_buf)
        //    .expect("Failed to read line");
        stream.read(&mut irc_msg_buf).unwrap();
        
        let mut irc_msg_string = String::from_utf8_lossy(&irc_msg_buf[..]);
        let () = irc_msg_string;
        if &irc_msg_string[irc_msg_string.len()-1..] == "\n" {
            irc_msg_string.truncate(irc_msg_string.len()-1);
        }

        let irc_msg_parsed = match parser::parse_message(&irc_msg_string) {
            Ok(msg) => msg,
            Err(msg) => {
                println!("Error: {:?}", msg);
                continue;
            }
        };

        if let Some(prefix) = irc_msg_parsed.opt_prefix {
            println!("got prefix!!");
            match prefix {
                parser::MsgPrefix::Name(name) => println!("nick/host: {}", name),
                parser::MsgPrefix::Nick(nick) => println!("nick: {}", nick),
                parser::MsgPrefix::NickHost(nick, host) => {
                    match host {
                        parser::HostType::HostName(host_name) =>
                            println!("nick, host: {}, {}", nick, host_name),
                        parser::HostType::HostAddrV4(ip_addr) =>
                            println!("nick, ipv4addr: {}, {}", nick, ip_addr),
                        parser::HostType::HostAddrV6(ip_addr) =>
                            println!("nick, ipv6addr: {}, {}", nick, ip_addr)
                    }
                }
                parser::MsgPrefix::NickUserHost(nick, user, host) => {
                    match host {
                        parser::HostType::HostName(host_name) =>
                            println!("nick, user, host: {}, {}, {}", nick, user, host_name),
                        parser::HostType::HostAddrV4(ip_addr) =>
                            println!("nick, user, ipv4addr: {}, {}, {}", nick, user, ip_addr),
                        parser::HostType::HostAddrV6(ip_addr) =>
                            println!("nick, ipv6addr: {}, {}, {}", nick, user, ip_addr)
                    }
                }
                parser::MsgPrefix::Host(host) => {
                    match host {
                        parser::HostType::HostName(host_name) =>
                            println!("host: {}", host_name),
                        parser::HostType::HostAddrV4(ip_addr) =>
                            println!("ipv4addr: {}", ip_addr),
                        parser::HostType::HostAddrV6(ip_addr) =>
                            println!("ipv6addr: {}", ip_addr)
                    }
                }
            }
        }

        println!("command is: {}", irc_msg_parsed.command);

        if let Some(params) = irc_msg_parsed.opt_params {
            println!("got some parameters");
            for item in params.iter() {
                println!("{}", item);
            }
        }
    }
}

