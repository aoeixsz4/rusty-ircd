// client
// this file contains futures, handlers and socket code for dealing with
// async IO for connected clients
extern crate tokio;
extern crate futures;

use crate::buffer;
use crate::irc;
use crate::parser;
use parser::ParseError;

use std::sync::{Mutex, Arc};
use std::net::SocketAddr;
use std::io::{Error, ErrorKind};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite};
use futures::{Future, Async, Poll, Stream};
use futures::task;
use futures::task::Task;
use crate::buffer::MessageBuffer;
use crate::irc::rfc_defs as rfc;
use crate::irc::Core;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use dns_lookup::lookup_addr;

pub struct ClientList {
    pub map: HashMap<u64, Arc<Mutex<Client>>>,
    pub next_id: u64
}

impl ClientList {
    pub fn new() -> Self {
        ClientList {
            map: HashMap::new(),
            next_id: 0
        }
    }
}

// this future is a wrapper to the Client struct, and implements the polling code
pub struct ClientFuture {
    pub client: Arc<Mutex<Client>>,
    pub id: u64, // same as client id
    pub first_poll: bool,
    pub irc_core: Core
}

impl ClientFuture {
    // call when client connection drops (either in error or if eof is received)
    // remove client from list on EOF or connection error
    fn unlink_client(&mut self, client: &Client) {
        let mut client_list = self.irc_core.clients.lock().unwrap();
        // HashMap::remove() returns an Option<T>, so we can either
        // ignore the possibility that the client is alread unlinked, or deliberately panic
        // (since if this fails, there may well be a bug elsewhere
        if let None = client_list.map.remove(&client.id) {
            panic!("client {} doesn't exist in our list, there is likely a bug somewhere");
        }
    }
    
    // to be called from polling future
    fn try_flush(&mut self, client: &mut Client) -> Result<usize, Error> {
        // now we also have the slightly annoying situation that if bytes_out < out_i,
        // we have to either do someething complicated with two indices, or shift
        // bytes to the start of the buffer every time a write completes
        let mut write_count: usize = 0;
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];

        // create a special scope where we use the Arc<Mutex<>> wrapper to copy stuff into 
        // our temporary write buffer
        let len = client.output.copy(&mut tmp_buf); // returns bytes copied

        // write as much as we can while just incrementing indices
        while write_count < len {
            match client.socket.poll_write(&tmp_buf[write_count .. len]) {
                Ok(Async::Ready(bytes_out)) => write_count += bytes_out, // track how much we've written
                Ok(Async::NotReady) => break,
                Err(e) => return Err(e)
            }
        }

        // if write_count > 0, get mutex again and shift bytes
        // (or just reset index if write_count == index
        if write_count > 0 {
            if write_count == client.output.index {
                client.output.index = 0;
            } else {
                client.output.shift_bytes_to_start(write_count);
            }
        }

        // only return Ready when it's time to drop the client
        Ok(write_count)
    }

    fn try_read(&mut self, client: &mut Client) -> Result<usize, Error> {
        // we'll read anything we can into a temp buffer first, then only later
        // transfer it to the mutex guarded client.output buffer
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let mut tmp_index: usize = 0;
        while tmp_index < rfc::MAX_MSG_SIZE { // loop until there's nothing to read or the buffer's full
            match client.socket.poll_read(&mut tmp_buf[tmp_index ..]) {
                Ok(Async::Ready(bytes_read)) if bytes_read == 0 =>
                    // EOF - this Future completes when the client is no more
                    // WARNING - possible edge case:
                    // client writes a valid message followed directly by EOF,
                    // we end up ignoring their message in the buffer
                    return Err(Error::new(ErrorKind::UnexpectedEof, "received EOF")),
                Ok(Async::Ready(bytes_read)) => tmp_index += bytes_read,
                Ok(Async::NotReady) => break, // can't read no more
                Err(e) => return Err(e)
            }
        }

        // now we have (potentially) filled some bytes in a temp buffer
        // get a mutex lock and update stuff
        if tmp_index > 0 {
            // if the below call returns an error, the client will be dropped
            client.input.append_bytes(&mut tmp_buf[.. tmp_index])?;
        }

        Ok(tmp_index)
    }

    // forward incoming message to other users
    fn broadcast(&self, map: &HashMap<u64, Arc<Mutex<Client>>>, msg: &str) {
        for (id, target) in map {
            // skip writing to ourself
            if *id == self.id {
                continue;
            }
            
            // get a mutex on the other client
            let mut target = target.lock().unwrap();

            // send_line() takes care of notifying the Future's task and flags
            // the client as dead if the append fails (indicates full buffer
            // (indicates flushes are not successful))
            target.send_line(&msg);
        }
    }
}

impl Future for ClientFuture {
    type Item = ();
    type Error = ();
    // this here is the main thing
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let client = Arc::clone(&self.client);
        let mut client = client.lock().unwrap();  // client is now under mutex lock

        // is connection/client dead? drop from list and return Ready to complete our future
        if client.dead == true {
            self.unlink_client(&client);
            return Ok(Async::Ready(()));
        }

        // if its the first time polling, we need to register our task
        if self.first_poll == true {
            self.first_poll = false;
            client.handler = task::current();
        }

        // try to write if there is anything in outbuf,
        // returns error if there is a connection problem, in which case drop the client
	if let Err(_e) = self.try_flush(&mut client) {
            self.unlink_client(&client);
            return Ok(Async::Ready(()));
        }

        // try to read into our client's in-buffer
        if let Err(_e) = self.try_read(&mut client) {
            self.unlink_client(&client);
            return Ok(Async::Ready(()));
        }

        // loop while client's input buffer contains line delimiters
        while client.input.has_delim() {
            let msg_string = client.input.extract_ln();
            assert!(msg_string.len() != 0);
            let parsed_msg_res = parser::parse_message(&msg_string);
            match parsed_msg_res {
                Ok(msg) => irc::handle_command(&mut self.irc_core, &mut client, msg),
                Err(parse_err) => match parse_err {
                    ParseError::InvalidPrefix => client.send_line("invalid prefix!"),
                    ParseError::NoCommand => client.send_line("no command given!"),
                    ParseError::InvalidCommand => client.send_line("invalid command!"),
                    ParseError::InvalidNick => client.send_line("invalid nick!"),
                    ParseError::InvalidUser => client.send_line("invalid user!"),
                    ParseError::InvalidHost => client.send_line("invalid host!")
                }
            }
        }
        Ok(Async::NotReady)
    }
}

pub enum ClientType {
    Unregistered,
    User(Arc<Mutex<irc::User>>),
    Server(Arc<Mutex<irc::Server>>),
    ProtoUser(Arc<Mutex<irc::ProtoUser>>)
}

pub struct Client { // is it weird/wrong to have an object with the same name as the module?
    // will need a hash table for joined channels
    //channels: type unknown
    pub socket: TcpStream,
    //flags: some sort of enum vector?
    //host: irc::Host,
    pub client_type: ClientType,
    pub id: u64,
    pub input: MessageBuffer,
    pub output: MessageBuffer,
    pub handler: Task,
    pub dead: bool // this will be flagged if poll() needs to remove the client
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
    pub fn new(id: u64, task: Task, socket: TcpStream) -> Client {
        Client {
            output: buffer::MessageBuffer::new(),
            input: buffer::MessageBuffer::new(),
            handler: task, // placeholder
            client_type: ClientType::Unregistered, // this will be established by a user or server handshake
            dead: false,
            socket,
            id
        }
    }

    // fn sends a line to the client - this function adds the cr-lf delimiter,
    // so just an undelimited line should be passed as a &str
    // the function also notifies the runtime that the socket handler needs
    // to be polled to flush the write
    pub fn send_line(&mut self, buf: &str) {
        let mut string = buf.to_string();
        string.push_str("\r\n");
        self.handler.notify();
        if let Err(_e) = self.output.append_str(&string) {
            self.dead = true;
        }
    }
}
