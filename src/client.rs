// client
// this file contains futures, handlers and socket code for dealing with
// async IO for connected clients
extern crate tokio;
extern crate futures;

use crate::buffer;
use crate::irc;
use crate::parser;

use std::sync::{Mutex, Arc};
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite};
use futures::{Future, Async, Poll, Stream};
use futures::task;
use futures::task::Task;
use crate::buffer::MessageBuffer;
use crate::irc::rfc_defs as rfc;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub enum ClientCommand {
    Empty
}

// this future is a wrapper to the Client struct, and implements the polling code
struct ClientFuture {
    client: Arc<Mutex<Client>>,
    client_list: Arc<Mutex<ClientList>>,
    id: u32, // same as client id
    first_poll: bool
}

impl Future for ClientFuture {
    type Item = ();
    type Error = ();
    
        // to be called from polling future
    fn try_flush(&mut self) -> Result<Self::Item, Self::Error>  {
        // now we also have the slightly annoying situation that if bytes_out < out_i,
        // we have to either do someething complicated with two indices, or shift
        // bytes to the start of the buffer every time a write completes
        let mut client = self.client.lock().unwrap();
        let mut write_count: usize = 0;
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];

        // create a special scope where we use the Arc<Mutex<>> wrapper to copy stuff into 
        // our temporary write buffer
        let len = {
            let outbuf = self.outbuf.lock().unwrap();
            if outbuf.index > 0 {
                outbuf.copy(&mut tmp_buf) // returns bytes copied
            } else {
                0
            }
        };

		// write as much as we can while just incrementing indices
        while write_count < len {
            match client.socket.poll_write(&tmp_buf[write_count .. len]) {
                Ok(Async::Ready(bytes_out)) => write_count += bytes_out, // track how much we've written
                Ok(Async::NotReady) => break,
                Err(e) => {
                    println!("connection error: {}", e);
                    let mut client_list = self.client_list.lock().unwrap();
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(()));
                }
            }
        }

        // if write_count > 0, get mutex again and shift bytes
        // (or just reset index if write_count == index
        if write_count > 0 {
            let outbuf = Arc::clone(&client.output);
            let mut outbuf = outbuf.lock().unwrap();
            if write_count == outbuf.index {
                outbuf.index = 0;
            } else {
                outbuf.shift_bytes_to_start(write_count);
            }
        }
    }

	// this here is the main thing
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let client = Arc::clone(&self.client);
        let mut client = client.lock().unwrap();  // client is now under mutex lock

        // if its the first time polling, we need to register our task
        if self.first_poll == true {
            self.first_poll = false;
            client.handler = task::current();
        }

        // try to write if there is anything in outbuf,
        // bad idea to loop here, unless perhaps we have a break statement on Async::NotReady?
		match self.try_flush() {
		}
                    
        // we'll read anything we can into a temp buffer first, then only later
        // transfer it to the mutex guarded client.output buffer
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let mut tmp_index: usize = 0;
        while tmp_index < rfc::MAX_MSG_SIZE { // loop until there's nothing to read or the buffer's full
            match client.socket.poll_read(&mut tmp_buf[tmp_index ..]) {
                Ok(Async::Ready(bytes_read)) if bytes_read == 0 => {
                    println!("eof");
                    // remove client from our map...
                    // need to aquire a mutex lock
                    let mut client_list = self.client_list.lock().unwrap();
                    // we already have a lock on our client in the outer scope
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(())); // this Future completes when the client is no more
                }
                Ok(Async::Ready(bytes_read)) => {
                    println!("Client wrote us!");
                    tmp_index += bytes_read;
                }
                Ok(Async::NotReady) => break,
                Err(e) => {
                    println!("connection error: {}", e);
                    // remove client from our map...
                    // need to aquire a mutex lock
                    let mut client_list = self.client_list.lock().unwrap();
                    // we already have a lock on our client in the outer scope
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(())); // this Future completes when the client is no more
                }
            }
        }

        // buffer never seems to overflow... what's going on?
        
        // now we have (potentially) filled some bytes in a temp buffer
        // get a mutex lock and update stuff
        if tmp_index > 0 {
            let inbuf = Arc::clone(&client.input);
            let mut inbuf = inbuf.lock().unwrap();
            match inbuf.append_bytes(&mut tmp_buf[.. tmp_index]) {
                Ok(()) => (), // do nothing
                Err(_e) => {
                    println!("buffer overflow! dropping connection");
                    let mut client_list = self.client_list.lock().unwrap();
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(()));
                }
            }
        }

        // ok - we can read, and also have untested write code above,
        // but nothing to write, so here we check the client input buffer,
        // for a cr-lf delimiter, and append to other client output buffers
        {
            let inbuf = Arc::clone(&client.input);
            let mut inbuf = inbuf.lock().unwrap();
            let client_list = self.client_list.lock().unwrap();
            while inbuf.has_delim() {
                println!("got delim!");
                let mut msg_string = inbuf.extract_ln();
                msg_string.push_str("\r\n");
                for (id, other_client) in &client_list.map {
                    if *id == client.id {
                        continue;
                    }
                    let mut other_client = other_client.lock().unwrap();
                    let mut outbuf = other_client.output.lock().unwrap();


                    // this is a bit tricky,
                    // currently doing it like this only writes next time the other client
                    // is poll()ed, which seems to happen only when the other client writes
                    // something. we need some way to register an event on the socket/future,
                    // so it knows we want to do something
                    // or, just do all our poll_writes down here
                    // need to read up more on the events driving the polling of futures
                    match outbuf.append_str(&msg_string) {
                        Ok(()) => (), // do nothing
                        Err(_e) => {
                            println!("buffer overflow! dropping connection");
                            let mut client_list = self.client_list.lock().unwrap();
                            client_list.map.remove(&client.id);
                            return Ok(Async::Ready(()));
                        }
                    }
                    
                    // need to notify ... this appears to do nothing
                    other_client.handler.notify();
                }
            }
        }
        Ok(Async::NotReady)
    }
}

pub struct Client { // is it weird/wrong to have an object with the same name as the module?
    // will need a hash table for joined channels
    //channels: type unknown
    //socket: SocketType,
    //flags: some sort of enum vector?
    host: irc::Host,
    inbuf: Arc<Mutex<MessageBuffer>>,
    outbuf: Arc<Mutex<MessageBuffer>>,
    handler: task::Task,
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
            host,
            outbuf: buffer::MessageBuffer::new(),
            inbuf: buffer::MessageBuffer::new(),
            handler: None, // set this later
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
        let parsed_msg = parser::parse_message(&command_string)?;

        // do something with the parsed message, irc.rs code needs to get involved
        Ok(ClientCommand::Empty)
    }

    // fn sends a line to the client - this function adds the cr-lf delimiter,
    // so just an undelimited line should be passed as a &str
    // the function also notifies the runtime that the socket handler needs
    // to be polled to flush the write
    pub fn send_line(&mut self, buf: &str) -> Result<(), buffer::BufferError> {
        let mut outbuf = self.output.lock().unwrap();
        let mut string = String::from_utf8_lossy(&buf).to_string();
        string.push_str("\r\n");
        output.append_str(string)?;
        self.handler.notify();
    }
}
