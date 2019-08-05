// client
// this file contains protocol defitions related to client commands coming to the server
use crate::buffer;
use crate::irc;
use crate::parser;
use std::net{IpAddr, Ipv4Addr, Ipv6Addr};

pub enum ClientCommand {
    Empty
}

pub struct Client { // is it weird/wrong to have an object with the same name as the module?
    // will need a hash table for joined channels
    //channels: type unknown
    //socket: SocketType,
    //flags: some sort of enum vector?
    host: irc::Host,
    inbuf: buffer::MessageBuffer,
    outbuf: buffer::MessageBuffer,
}

// externally, clients will probably be collected in a vector/hashmap somewhere
// each client will have a unique identifier: their host (type irc::Host),
// each must have a socket associated with it
// clients here mean something associated with a socket connection -
// i.e. they can be servers or users
// somewhere we'll need code for mapping external users to whatever
// relay server we can reach them through
impl Client {
    // since new clients will be created on a new connection event,
    // we'll need a socket type as a parameter
    // implementation decision: explicitly return as a pointer to heap data
    // will also be necessary that all threads can access every client object
    pub fn new(host: irc::Host) -> Client {
        Client {
            mut host,
            mut outbuf: super::MessageBuffer::new(),
            mut inbuf: super::MessageBuffer::new(),
            //socket,
        }
    }

    // an event handler waiting on new data from the client
    // must call this handler when a CR-LF is found
    // return type is a ClientCommand, which will be processed elsewhere
    pub fn end_of_line(&mut self) -> Result<ClientCommand, parser::ParseError> {
        // NB: buffer index might not be directly after the CR-LF
        // first bit of code locates a CR-LF
        // next bit extracts a string and moves buffer data after CR-LF
        // to front, reseting the index afterwards
        let command_string = self.inbuf.extract_ln();

        // i will insist that the event handler doesn't hand us empty lines
        assert!(command_string.len() != 0);
        let parsed_msg = parser::parse_message(command_string)?;

        // do something with the parsed message, irc.rs code needs to get involved
    }
}
