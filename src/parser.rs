// this module deals with parsing IRC message strings, and returning
// a Vec<&str> of parameters etc. to pass to the main protocol handlers
// earlier vesions of this code dealt too much with things specific to certain commands
// the parsers job should instead ensure messages conform to the standard structure
// as defined in Augmented BNF in the RFC 2812, but without ensuring
// that command-specific restrictions are adhered to (that will be done elsewhere in irc or
// irc::command or so)
// link: https://tools.ietf.org/html/rfc2812#section-2.3.1
// plus an optional source field (for server messages, indicating origin)
use crate::irc;
use crate::irc::rfc_defs as rfc;

// will want to change these types at some point
#[derive(Debug)]
pub enum ParseError {
    CommandNotRecognised(String),
    TooFewArgs(String),
    MissingCRLF,
    InvalidPrefix,
    NoCommand,
    EmptyLine
}

// 
fn parse_prefix(msg: &str) -> Result<Box<irc::Source>, ParseError> {
    // we should probably also return an error if the space occurs immediately after the colon
    if space_index == 0 {
    	return (msg, Err(ParseError::InvalidPrefix));
    }
    
    // split into two new slices
    let prefix = &msg[..space_index];
    let rest = &msg[space_index+1..];
    
    // check prefix confirms to the correct format servername / (nick [ [ ! user ] @ host])
    let ex_matches: Vec<_> = prefix.match_indices('!').collect();
    let at_matches: Vec<_> = prefix.match_indices('@').collect();
    if  ex_matches.len() > 1        // max one !
        || at_matches.len() > 1     // max one @
        || (
            ex_matches.len() == 1
            &&  (
                    at_matches.len() == 0   // nick!user is not valid
                                            // nick@user!host is also not valid
                    || (at_matches.len() == 1 && ex_matches[0].0 > at_matches[0].0)
                )
            ) {
        return (msg, Err(ParseError::InvalidPrefix));
    }
        
    //     
    let prefix_parts: Vec<&str> = prefix.split(|c| c == '@' || c == '!').collect();

    // usually server
    // according to the RFC, :nick is also a valid prefix,
    // but I don't know how else to distinguish server and user
    // except by the fact users usually have :nick!user@host
    let name = String::from(&prefix_parts[0][..]);

    // at this point length cannot be anything other than 1, 2 or 3
    if prefix_parts.len() == 1 {
        let my_box: Box<irc::Source> = Box::new(irc::Source::Server(name));
        (rest, Ok(my_box))
    } else {
        let hostname = Some(String::from(&prefix_parts[1][..]));
        let username =  if prefix_parts.len() == 3 {
                            Some(String::from(&prefix_parts[2][..]))
                        } else { None };
        let my_box: Box<irc::Source> = Box::new(irc::Source::User(name, username, hostname));
        (rest, Ok(my_box))
    }
}
    

// this lil function snatches up a word and returns the rest of the string
// in an Option<String>, or just gives back the original String plus a None
fn split_colon_arg(msg: &str) -> (&str, Option<&str>) {
    if let Some(tail) = msg.find(" :") {
        (&msg[..tail], Some(&msg[tail+2..]))
    } else {
        (msg, None)
    }
}

// parsing IRC messages :)
// we'll also take ownership, calling function shouldn't need the original string anymore
// irc::Message will contain both the Command and the Source (tho the latter is sometimes absent
// we want to use mostly &str operations for the parsing itself,
// but we don't want to have to care about the fate of message,
// so all the data structures we return will have new owned Strings
//    Augmented BNF notation for general message strcture
//    message    =  [ ":" prefix SPACE ] command [ params ] crlf
pub fn parse_message(mut message: &str) -> Result<ParsedMsg, ParseError> {
    // make sure the message format is on point
    if message.len() <= 2 {
        return Err(ParseError::EmptyLine);
    }

    // deal with CRLF
    let crlf = &message[message.len()-2..];
    if crlf != "\r\n" {
        return Err(ParseError::MissingCRLF);
    }
    message = &message[..message.len()-2];

    // check for a message prefix
    // should be bounded by a ':' at the very start of the string
    // and a
    let mut origin: Option<&irc::Source> = None;

    if message.bytes().nth(0) == ':' {
        // check for a space
        if let Some(index) = message.find(' ') {
            let prefix = &message[1..index];
            if prefix.len() == 0 {
                return Err(ParseError::InvalidPrefix);
            }
            message = &message[index+1..];
            match parse_prefix(&prefix) {
                Ok(val) => origin = Some(val),
                Err(err) => return Err(err)
            }
        }
    }
    let (message, colon_arg) = split_colon_arg(&message);
    let n = rfc::MAX_MSG_PARAMS + 1;
    if let Some(arg) = colon_arg {
            n -= 1;
    }
    let mut args: Vec<&str> = message.splitn(' ', n).collect();
    if let Some(arg) = colon_arg {
        args.push(arg);
    }
    let command = args.remove(0);

    // command_type is an enum, but needs its parameters filled
    // also need some checks for things like optional params
    match command {
        "JOIN" => {
            // error if not enough args
            if args.len() < 1 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }
            
            // anything else will be ignored, JOIN only needs a chan argument
            // arg strings will be cloned before passing to Command::type(),
            // otherwise we will have lifetime problems, and we can't move stuff
            // from the args vector - this way args will cleanly go out of scope
            let channel = args[0].to_string();

            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::Join(channel)),
                src: prefix
            })
        }
        "MSG" => {
            if args.len() < 2 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }
            
            let target = args[0].to_string();
            let message = args[1].to_string();

            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::Message(target, message)),
                src: prefix
            })
        }
        "PART" => {
            // error if no chan given
            if args.len() < 1 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }

            let channel = args[0].to_string();
            // anything in rest will be ignored, JOIN only needs a chan argument

            // Option<String> is the expected type for Command::Part.part_message
            let mut part_message: Option<String> = None;
            if args.len() > 1 {
                part_message = Some(args[1].to_string()); // don't forget to wrap the Option<T>s
            }

            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::Part(channel, part_message)),
                src: prefix
            })
        }
        "NICK" => {
            // error if no nick given
            if args.len() < 1 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }

            let nick = args[0].to_string();

            // prepare return struct
            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::Nick(nick)),
                src: prefix
            })
        }
        "USER" => {
            // total of 4 args required
            if args.len() < 4 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }
            
            let username = args[0].to_string();

            // not sure if silently ignoring non-numbers
            // for the mode field is canonical behaviour,
            // but whatever
            let mode = match args[1].parse::<u32>() {
                Ok(val) => val,
                Err(_) => 0
            };


            // USER is specified as having an unused field, usually a * is supplied there
            //let ignored = args[2]
            let real_name = args[3].to_string();
            
            // prepare return struct
            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::User(username, mode, real_name)),
                src: prefix
            })
        }
        "QUIT" => {
            let quit_msg = if args.len() > 0 {
                Some(args[0].to_string())
            } else {
                None
            };
            
            // prepare return struct
            Ok(irc::Message {
                cmd_params: Box::new(irc::Command::Quit(quit_msg)),
                src: prefix
            })
        }
        _ => Err(ParseError::CommandNotRecognised(command.to_string()))
    }
}
