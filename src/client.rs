// client
// this file contains futures, handlers and socket code for dealing with
// async IO for connected clients
extern crate tokio;
extern crate futures;

use crate::buffer;
use crate::irc;
use crate::parser;
use parser::ParseError;

use std::sync::{Arc, Mutex, Weak};
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

// this future is a wrapper to the Client struct, and implements the polling code
pub struct ClientFuture {
    pub client: Arc<Mutex<Client>>,
    pub id: u64, // same as client id
    pub first_poll: bool,
    pub irc_core: Core
}

impl ClientFuture {
    // to be called from polling future
    fn try_flush(&mut self) -> Result<usize, Error> {
        // now we also have the slightly annoying situation that if bytes_out < out_i,
        // we have to either do someething complicated with two indices, or shift
        // bytes to the start of the buffer every time a write completes
        
        // try to get a lock on the client, non-blocking style
        let mut client = self.client.lock().unwrap();

        let mut write_count: usize = 0;
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let len = client.output.copy(&mut tmp_buf); // returns bytes copied

        // write as much as we can while just incrementing indices
        while write_count < len {
            match client.socket.poll_write(&tmp_buf[write_count .. len]) {
                Ok(Async::Ready(bytes_out)) => write_count += bytes_out, // track how much we've written
                Ok(Async::NotReady) => break,
                Err(e) => return Err(e)
            }
        }

        // if write_count > 0, shift bytes
        // (or just reset index if write_count == index
        if write_count > 0 {
            if write_count == client.output.index {
                client.output.index = 0;
            
                client.output.shift_bytes_to_start(write_count);
            }
        }

        // only return Ready when it's time to drop the client
        Ok(write_count)
    }

    fn try_read(&mut self) -> Result<usize, Error> {
        // we'll read anything we can into a temp buffer first, then only later
        // transfer it to the mutex guarded client.output buffer
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let mut tmp_index: usize = 0;

        // try (non-blocking) to get lock on the client
        let mut client = self.client.lock().unwrap();

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

    // call when client connection drops (either in error or if eof is received)
    // remove client from list on EOF or connection error
    fn unlink_client(&mut self) {
        // HashMap::remove() returns an Option<T>, so we can either
        // ignore the possibility that the client is alread unlinked, or deliberately panic
        // (since if this fails, there may well be a bug elsewhere
        // client hash map is actually less important than some of the other stuff
        // removing the Strong Arc pointers from ClientFuture will be the main thing
        // ClientFuture has a strong Arc pointer to Client, which has a strong
        // Arc pointer to User, and User has Strong Arc pointers to the hashes
        // in Core, but they only contain weak pointers, so I think we're in the
        // clear
        // we should probably notify the irc core code to do something
        // about a dropped client, but most of the memory stuff will
        // be dealt with purely by ClientFuture leaving scope

        // NOTICE_BLOCKY - however, tokio recommend using normal,
        // blocking mutexes unless you need to hold locks across
        // .await calls, so maybe this is fine
        self.irc_core.clients.lock().unwrap().remove(&self.id);
    }
}

impl Future for ClientFuture {
    type Item = ();
    type Error = ();
    // this here is the main thing
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // is connection/client dead? drop from list and return Ready to complete our future
        if self.client.lock().unwrap().is_dead() {
            self.unlink_client();
            return Ok(Async::Ready(()));
        }

        if self.first_poll == true {
            self.first_poll = false;
            self.client.lock().unwrap().set_handler(task::current());
        }

        // try to write if there is anything in outbuf,
        // returns error if there is a connection problem, in which case drop the client
	    if let Err(_e) = self.try_flush() {
            self.unlink_client();
            return Ok(Async::Ready(()));
        }

        // try to read into our client's in-buffer
        if let Err(_e) = self.try_read() {
            self.unlink_client();
            return Ok(Async::Ready(()));
        }

        // loop while client's input buffer contains line delimiters
        loop {
            // by using the scope trick like this we don't keep
            // the mutex lock after assigning to line
            let try_line = self.client.lock().unwrap().try_get_line();

            if let Some(message) = try_line {
                let responses = match parser::parse_message(&message) {
                    Ok(msg) => irc::command(self.irc_core.clone(), Arc::clone(&self.client), msg),
                    Err(parse_err) => {
                        let resp = Vec::new();
                        resp.push(parser::err_to_msg(parse_err));
                        resp
                    }
                };

                for response in responses {
                    self.client.lock().unwrap().send_line(&response);
                }
            } else {
                break;
            }
        }

        Ok(Async::NotReady)
    }
}

pub enum ClientType {
    Unregistered,
    User(Arc<Mutex<irc::User>>),
    //Server(Arc<Mutex<irc::Server>>), leave serv implmentation for much later
    ProtoUser(Arc<Mutex<irc::ProtoUser>>)
}

impl Clone for ClientType {
    fn clone (&self) -> Self {
        match self {
            ClientType::Unregistered => ClientType::Unregistered,
            ClientType::User(user_ptr) =>
                ClientType::User(Arc::clone(user_ptr)),
            ClientType::ProtoUser(proto_user_ptr) =>
                ClientType::ProtoUser(Arc::clone(proto_user_ptr))
        }
    }
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

    pub fn set_handler(&mut self, task: Task) {
        self.handler = task;
    }

    // fn sends a line to the client - this function adds the cr-lf delimiter,
    // so just an undelimited line should be passed as a &str
    // the function also notifies the runtime that the socket handler needs
    // to be polled to flush the write
    pub fn send_line(&mut self, line: &str) {
        self.handler.notify();
        if let Err(_e) = self.output.append_ln(&line) {
            self.dead = true;
        }
    }

    /* just a wrapper for extract_ln()... it checks for
     * a delimiter and returns None if there isn't one anyway */
    pub fn try_get_line(&self) -> Option<String> {
        self.input.extract_ln()
    }

    pub fn is_dead(&self) -> bool {
        self.dead
    }
}
