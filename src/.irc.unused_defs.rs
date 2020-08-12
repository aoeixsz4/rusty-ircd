// I hope it doesnt get too confusing that parser.rs and irc.rs both have a 'Host' enum,
// main difference is the parser's variants only contain strings (either for hostname
// or ip address), the type here will contain a hostname string or a proper ip address type
// from std::net
/*pub enum Origin {
    Server(Host),
    User(String, Option<String>, Option<Host>)
}*/
/*
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
    Join(Vec<String>), // #channel
    Part(Vec<String>, Option<String>), // #channel, part-message
    Message(Vec<String>, String), // dest (user/#channel), message
    Nick(String), // choose nickname
    User(String, u64, String), // choose username (might need addition parameters)
    Quit(Option<String>), // quit-message
}*/

/*pub struct ServerUserFlags {
    server_op: bool
}*/

/*pub struct ServerUser {
    nick: String,
    flags: ServerUserFlags
}

pub struct Server {
    id: u64,
    host: Host,
    users: Vec<ServerUser>,
    client_id: u64
}*/

// flags related to user priveleges in a specific channel
/*pub struct ChanUserFlags {
    chan_op: bool,
    chan_halfop: bool,
    chan_voice: bool
}

pub struct ChanUser {
    nick: String,
    flags: ChanUserFlags
}

// channel needs a name, a topic, and a list of joined users
// this list can't just be a list of nicks, as additional flags are required: is the user an op on
// the channel, for example?
pub struct Channel {
    name: String,
    users: Vec<ChanUser>,
    topic: String
}*/
