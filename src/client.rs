// irc::client
// this file contains protocol defitions related to client commands coming to the server

pub struct Client { // is it weird/wrong to have an object with the same name as the module?
    // will need a hash table for joined channels
    nick: String,
    username: String,
    //channels: type unknown
    socket: SocketType,
    //flags: some sort of enum vector?
    inbuf: super::MessageBuffer,
    outbuf: super::MessageBuffer,
}

// externally, clients will probably be collected in a vector somewhere
impl Client {
    // since new clients will be created on a new connection event,
    // we'll need a socket type as a parameter
    pub fn new(socket: SocketType) -> Client {
        Client {
            mut nick: String::new(),
            mut username: String::new(),
            mut out_buffer: super::MessageBuffer,
            mut in_buffer: super::MessageBuffer,
            socket,
        }
    }

    // an event handler waiting on new data from the client
    // must call this handler when a CR-LF is found
    // return type is a ClientCommand, which will be processed elsewhere
    pub fn end_of_line(&mut self) -> ClientCommand {
        // NB: buffer index might not be directly after the CR-LF
        // first bit of code locates a CR-LF
        // next bit extracts a string and moves buffer data after CR-LF
        // to front, reseting the index afterwards
        let command_string = self.out_buffer.extract();
    }
}
