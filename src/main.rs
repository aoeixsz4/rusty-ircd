mod irc;
use crate::irc::*;
use std::io;
use crate::irc::extract_prefix;

fn main () {
    loop {
        let mut irc_msg_raw = String::new();
        let mut irc_msg_slice;
        io::stdin().read_line(&mut irc_msg_raw)
            .expect("Failed to read line");
        
        //irc_msg.extend();
        irc_msg_slice = &irc_msg_raw[..];
        //let ex_res: Result<Box<irc::Source>, irc::ParseError>;
        let (irc_msg_slice, ex_res) = extract_prefix(irc_msg_slice);

//        let irc_command_full;
//        match ex_res {
//            Some(irc_msg) => irc_command_full = irc_msg,
//            Err(_) => {
//                    println!("ERROR!");
//                    continue;
//            }
//        }

        // first we have to unwrap the Result<T, E>
        let src_message: Box<irc::Source>;
        match ex_res {
            Ok(source) => src_message = source,
            Err(err_type) => {
                println!("Error: {:?}", err_type);
                continue;
            }
        }

        // now we can unwrap the actual source variants
        match *src_message {
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
        println!("message slice remaining: {}", irc_msg_slice);
    }
}

