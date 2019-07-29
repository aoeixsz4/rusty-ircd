mod irc;
use irc::*;
use std::io;

fn main () {
    let mut irc_msg = String::new();
    loop {
        io::stdin().read_line(&mut irc_msg)
            .expect("Failed to read line");
        
        //irc_msg.extend();
        if let Ok(irc_msg, source) = extract_prefix(irc_msg) {
            match source {
                Source::Server(server_name) => println!("you are server: {}", server_name);
                Source::User(nick, user, host) => {
                    println!("Hi, {}!", nick);
                    if let Some(name) = user {
                        println!("Your username is {}", name);
                    }
                    if let Some(name) = host {
                        println!("Your hostname is {}", host);
                    }
                }
            }
        }
    }
}

