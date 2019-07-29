mod irc;
use crate::irc::*;
use std::io;
use crate::irc::parse_message;

fn main () {
    loop {
        let mut irc_msg_buf = String::new();
        
        // this gives us a String ending in LF, we want CR-LF
        io::stdin().read_line(&mut irc_msg_buf)
            .expect("Failed to read line");
        
        if &irc_msg_buf[irc_msg_buf.len()-1..] == "\n" {
            irc_msg_buf.truncate(irc_msg_buf.len()-1);
        }
        irc_msg_buf.extend("\r\n".chars());

        let irc_msg_parsed = match parse_message(&irc_msg_buf[..]) {
            Ok(cmd) => cmd,
            Err(msg) => {
                println!("Error: {:?}", msg);
                continue;
            }
        };

        match *(irc_msg_parsed.cmd_params) {
            irc::Command::Join(chan) =>
                println!("you said: JOIN: {}", chan),
            irc::Command::Nick(nick) =>
                println!("you change your nick to {}", nick),
            irc::Command::User(user, mode, real_name) => {
                println!("sup, {}, your mode is {}", real_name, mode);
                println!("welcome to the irc_daemon, {}", user);
            }
            irc::Command::Quit(msg) => {
                println!("bye!");
                if let Some(value) = msg {
                    println!("you left a message: {}", value);
                }
            }
            irc::Command::Part(chan, msg) => {
                if let Some(value) = msg {
                    println!("now leaving channel {}, your part message: {}", chan, value);
                } else {
                    println!("now leaving channel: {}", chan);
                }
            }
            irc::Command::Message(dest, msg) =>
                println!("you send message '{}' to {}", msg, dest),
            irc::Command::Null =>
                println!("received empty line")
        }

        if let Some(source) = irc_msg_parsed.src {
            // now we can unwrap the actual source variants
            match *source {
                Source::Server(server_name) => println!("you are server: {}", server_name),
                Source::User(nick, user, host) => {
                    println!("Hi, {}!", nick);
                    if let Some(name) = user {
                        println!("Your username is {}", name);
                    }
                    if let Some(name) = host {
                        println!("Your hostname is {}", name);
                    }
                }
            }
        }
    }
}

