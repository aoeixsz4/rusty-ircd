// this module will contain various definitions taken directly
// from the official IRC protocols (RFCs 1459, 2812)
// e.g. an enum type IRC_Command, which will include possible commands,
// a communication buffer type used for server<->client communication


const MESSAGE_SIZE: usize = 512;
pub enum BufferError {
    OverFlow,
}

pub enum Source {
    Server(String),
    User(String, Option<String>, Option<String>)
}

#[derive(Debug)]
pub enum ParseError {
    CommandNotRecognised(String),
    TooFewArgs(String),
    MissingCRLF,
    InvalidPrefix,
    NoCommand
}

pub fn extract_prefix(msg: &str) -> (&str, Result<Box<Source>, ParseError>) {
    let space_index = match msg.find(' ') {
        Some(space) => space,
        None => return (msg, Err(ParseError::NoCommand))
    };
    
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
        let my_box: Box<Source> = Box::new(Source::Server(name));
        (rest, Ok(my_box))
    } else {
        let hostname = Some(String::from(&prefix_parts[1][..]));
        let username =  if prefix_parts.len() == 3 {
                            Some(String::from(&prefix_parts[2][..]))
                        } else { None };
        let my_box: Box<Source> = Box::new(Source::User(name, username, hostname));
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

//pub enum CmdType {
//    Join,
//    Part,
//    Message,
//    Nick,
//    User,
//    Quit,
//    Channel
//}

pub enum Command {
    Join(String), // #channel
    Part(String, Option<String>), // #channel, part-message
    Message(String, String), // dest (user/#channel), message
    Nick(String), // choose nickname
    User(String, u32, String), // choose username (might need addition parameters)
    Quit(Option<String>), // quit-message
    Null // empty line
}

pub struct IRCMessage {
//    cmd: CommandType,
    cmd_params: Box<Command>,
    src: Option<Box<Source>>
}

// parsing IRC messages :)
// we'll also take ownership, calling function shouldn't need the original string anymore
// IRCMessage will contain both the Command and the Source (tho the latter is sometimes absent
pub fn parse_message(mut message: &str) -> Result<IRCMessage, ParseError> {
    // make sure the message format is on point
    let crlf = &message[message.len()-2..];
    if crlf != "\r\n" {
        return Err(ParseError::MissingCRLF);
    } 
    message = &message[..message.len()-2];

    // I forgot how IRC protocol actually works
    // first we need to check for a colon (but only do anything with the first one)
    // anything before is space-separated, everything after the colon is a single argument,
    // usually a message
    let prefix = if &message[..1] == ":" {
        message = &message[1..];
        let (msg, resultant) = extract_prefix(&message);
        message = msg;

        // need to wrap return value in a Some
        Some(match resultant {
                Ok(value) => value,
                Err(error_string) => return Err(error_string)
        })
    } else { None };
    let (message, colon_arg) = split_colon_arg(&message);
    let mut args: Vec<&str> = message.split(' ').collect();
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
            let channel = String::from(args[0]);

            Ok(IRCMessage {
                cmd_params: Box::new(Command::Join(channel)),
                src: prefix
            })
        }
        "PART" => {
            // error if no chan given
            if args.len() < 1 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }

            let channel = String::from(args[1]);
            // anything in rest will be ignored, JOIN only needs a chan argument

            // Option<String> is the expected type for Command::Part.part_message
            let mut part_message: Option<String> = None;
            if args.len() > 2 {
                part_message = Some(String::from(args[2]));
            }

            Ok(IRCMessage {
                cmd_params: Box::new(Command::Part(channel, part_message)),
                src: prefix
            })
        }
        "NICK" => {
            // error if no nick given
            if args.len() < 1 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }

            let nick = String::from(args[1]);

            // prepare return Box
            Ok(IRCMessage {
                cmd_params: Box::new(Command::Nick(nick)),
                src: prefix
            })
        }
        "USER" => {
            // total of 4 args required
            if args.len() < 4 {
                return Err(ParseError::TooFewArgs(command.to_string()));
            }
            
            let username = String::from(args[0]);

            // not sure if silently ignoring non-numbers
            // for the mode field is canonical behaviour,
            // but whatever
            let mode: u32 = match args[1].parse() {
                Ok(mode_number) => mode_number,
                Err(_) => 0
            };


            // USER is specified as having an unused field, usually a * is supplied there
            //let ignored = args[2]
            let real_name = String::from(args[3]);
            
            // prepare return Box
            Ok(IRCMessage {
                cmd_params: Box::new(Command::User(username, mode, real_name)),
                src: prefix
            })
        }
        _ => Err(ParseError::CommandNotRecognised(command.to_string()))
    }
}


// might not always want this public
pub struct MessageBuffer {
    // the IRC protocol defines a maximum message size of 512 bytes,
    // including CR-LF. This being the case it doesn't make sense to
    // use buffers that resize as the client sends data, having a fixed
    // size will be generally faster due to simplified memory management
    buffer: [char; MESSAGE_SIZE],  // this needs to be char for String::extend() to work with a slice
    pub index: usize, // for incoming buffers we need some type of error handling
            // if we reach the end of the buffer and don't find CR-LF
}

impl MessageBuffer {
    // here we want to return the index of the next line, *after* CR-LF
    // so that the extract() fn spits out a string complete with carriage return
    // that will be stripped somewhere else in the program before/during message parsing
    fn get_eol(&self) -> Option<usize> {
        // anything past self.index is old (invalid!) data
        for i in 1..self.index {
            // byte literals are u8
            if self.buffer[i-1] == (b'\r' as char) && self.buffer[i] == (b'\n' as char) {
                return Some(i+1)
            }
        }
        None
    }

    // necessary to explicitly code for case where index is out of bounds? 
    // Rust should detect it and panic, I suppose
    fn shift_bytes_to_start(&mut self, start_index: usize) {
        // there's no need to copy everything to the very end of the buffer,
        // if it hasn't been completely filled
        for (i, j) in (start_index..self.index).enumerate() {
            self.buffer[i] = self.buffer[j];
        }
        self.index = self.index - start_index;  // there was a bug here! index should be shifted, not reset
    }

    // we only need this for client input buffers, so
    // might make more sense to implement in ClientIO?
    // then again its a task performed on the message buffer
    // and may prove to be more general
    // this probably should only be called when we know there's a CR-LF
    // to be found, but just incase we treat the no CR-LF case as
    // "return whatever string happens to currently be in there"
    pub fn extract(&mut self) -> Option<String> {
        if self.index == 0 {
            return None;
        }
        let mut out_string = String::new();
        if let Some(eol_index) = self.get_eol() {
            out_string.extend(&self.buffer[0..eol_index]);
            self.shift_bytes_to_start(eol_index);
        } else {
            out_string.extend(&self.buffer[..self.index]);
            self.index = 0;
        }
        Some(out_string)
    }

    // we also want code for appending to these buffers, more for server-> client writes
    // this can fail if the buffer doesn't have room for our message (probably indicates a connection problem)
    // for client buffers we're reading, this might be called by the incoming socket data event handler
    pub fn append(&mut self, message_string: String) -> Result<(), BufferError> {
        // how much space is left in the buffer?
        // does it make sense to try a partial write?
        if message_string.len() > (MESSAGE_SIZE - self.index) {
            return Err(BufferError::OverFlow);
        }
        for &byte in message_string.as_bytes() {
            self.buffer[self.index] = byte as char;
            self.index += 1;
        }
        return Ok(()); // returning Ok(current_index) as an output might be an option
    }

    pub fn new() -> MessageBuffer {
        MessageBuffer {
            buffer: [0 as char; MESSAGE_SIZE],
            index: 0,
        }
    }
}    

//mod client;
