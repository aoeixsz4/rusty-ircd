// this module will contain protocol definitions
// and core data types and handlers for IRC commands
//use crate::parser;

pub enum Source {
    Server(String),
    User(String, Option<String>, Option<String>)
}

// having to define and match these types here,
// as well as matching the raw command strings in parser::parse_message()
// feels somehow not quite right, but I haven't yet got a more elegant solution
// maybe parser should just hand over an Option<String> containing the src/origin prefix,
// followed by a vector reference of String args
// but the construction of the enum / match case is still clumsy,
// I want an elegant and fast way to go from a string literal to an enum...
// even with a match to string literals, I think it makes more sense to
// have the specific command cases handled here, and not have to change lots of things both
// here and in parser() if we want to add/change how commands work
pub enum Command {
    Join(String), // #channel
    Part(String, Option<String>), // #channel, part-message
    Message(String, String), // dest (user/#channel), message
    Nick(String), // choose nickname
    User(String, u32, String), // choose username (might need addition parameters)
    Quit(Option<String>), // quit-message
}

pub struct Message {
    pub cmd_params: Box<Command>,// commands and their parameters are defined as an enum above
    pub src: Option<Box<Source>> // important when servers relay commands originating from their users
}

