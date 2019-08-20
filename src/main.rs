extern crate tokio;
extern crate futures;
#[macro_use]
mod irc;
mod buffer;
mod client;
mod parser;

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

struct Client {
    socket: TcpStream,
    // judging by the compiler errors, i think I will also need to wrap these fuckers in
    // Arc<Mutex<>>
    input: MessageBuffer,
    output: MessageBuffer,
    handler: Task,
    id: u32
}

struct ClientFuture {
    client: Arc<Mutex<Client>>,
    client_list: Arc<Mutex<ClientList>>,
    id: u32, // same as client id
    first_poll: bool
}

struct ClientList {
    map: HashMap<u32, Arc<Mutex<Client>>>,
    next_id: u32
}

// will want to at some point merge this with existing client and messagebuffer code in client.rs
// and buffer.rs
impl Future for ClientFuture {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut client = self.client.lock().unwrap();  // client is now under mutex lock

        // if its the first time polling, we need to register our task handler
        if self.first_poll == true {
            self.first_poll = false;
            client.handler = task::current();
        }

        // try to write if there is anything in client.input,
        let mut write_count: usize = 0;
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let len = client.output.copy(&mut tmp_buf); // returns bytes copied

        while write_count < len {
            // borrowing rules don't allow us to pass &client.output.buf to
            // client.socket.poll_write(), hence the temp buffer on stack
            match client.socket.poll_write(&tmp_buf[write_count .. len]) {
                Ok(Async::Ready(bytes_out)) => write_count += bytes_out, // track how much we've written
                Ok(Async::NotReady) => break,
                Err(e) => {
                    println!("connection error: {}", e);
                    // would be nice if we could do this automatically whenever a ClientFuture
                    // completes
                    let mut client_list = self.client_list.lock().unwrap();
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(()));
                }
            }
        }

        // if write_count > 0, shift bytes
        // (or just reset index if write_count == index
        if write_count > 0 {
            if write_count == client.output.index {
                client.output.index = 0;
            } else {
                client.output.shift_bytes_to_start(write_count);
            }
        }

                    
        // we'll read anything we can into a temp buffer first
        // then copy it to the client.input
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];
        let mut tmp_index: usize = 0;
        while tmp_index < rfc::MAX_MSG_SIZE { // loop until there's nothing to read or the buffer's full
            match client.socket.poll_read(&mut tmp_buf[tmp_index ..]) {
                Ok(Async::Ready(bytes_read)) if bytes_read == 0 => {
                    println!("eof");
                    // remove client from our map...
                    // need to aquire a mutex lock
                    // again, nice if we can do this automatically on ClientFuture completion
                    let mut client_list = self.client_list.lock().unwrap();
                    // we already have a lock on our client in the outer scope
                    client_list.map.remove(&client.id);
                    return Ok(Async::Ready(())); // this Future completes when the client is no more
                }
                Ok(Async::Ready(bytes_read)) => tmp_index += bytes_read,
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

        // now we have (potentially) filled some bytes in a temp buffer
        if tmp_index > 0 {
            match client.input.append_bytes(&mut tmp_buf[.. tmp_index]) {
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
        while client.input.has_delim() {
            let client_list = self.client_list.lock().unwrap();
            let mut msg_string = client.input.extract_ln();
            msg_string.push_str("\r\n");
            for (id, other_client) in &client_list.map {
                // skip rather than echo back to same client
                if *id == client.id {
                    continue;
                }

                // mutex lock target client
                let mut other_client = other_client.lock().unwrap();

                // append to message buffer
                match other_client.output.append_str(&msg_string) {
                    Ok(()) => (), // do nothing
                    Err(_e) => {
                        println!("buffer overflow! dropping connection");
                        let mut client_list = self.client_list.lock().unwrap();
                        client_list.map.remove(&client.id);
                        return Ok(Async::Ready(()));
                    }
                }

                // notify the runtime so that target client's ClientFuture is polled
                other_client.handler.notify();
            }
        }
        Ok(Async::NotReady)
    }
}

fn process_socket(sock: TcpStream, clients: Arc<Mutex<ClientList>>) -> ClientFuture {
    // borrow checker complains about mutex locked clients
    // borrowing stuff when a move happens
    // so we can deliberately descope it before that
    // scope id here to use later
    let mut id = 0;
    let client = {
        let mut clients = clients.lock().unwrap();
        id = clients.next_id;
        let client = Arc::new(Mutex::new(Client {
            socket: sock,
            input: MessageBuffer::new(),
            output: MessageBuffer::new(),
            // placeholder, first poll() call to parent ClientFuture sets the real handler
            handler: task::current(),
            id
        }));
        // actual hashmap is inside ClientList struct
        clients.map.insert(id, Arc::clone(&client));

        // increment id value, this will only ever go up, integer overflow will wreak havoc,
        // but i doubt we reach enough clients for this to happen - should
        // be handled in any final release of code though, *just in case*
        clients.next_id += 1;   
        println!("client connected: id = {}", clients.next_id);
        client
    }; // drop mutex locked clients list
    // here we still keep ownership of a fresh Arc<Mutex<ClientList>> from outside this fn call
    // we can now return a future containing client data and a pointer to the client list,
    // and we don't have any reference cycles
    ClientFuture {
        client: Arc::clone(&client),
        client_list: clients,
        id,
        first_poll: true
    }
}
        

fn main() {
    let addr = "127.0.0.1:6667".parse::<SocketAddr>().unwrap();
    // Ups, this will just try to connect to above address,
    // we want to bind to it
    let listener = TcpListener::bind(&addr).unwrap();
    let clients = Arc::new(Mutex::new(ClientList {
        next_id: 0,
        map: HashMap::new()
    }));

    let server = listener.incoming()
        .map_err(|e| println!("failed to accept stream, error = {:?}", e))
        .for_each(move |sock| {
            // clone needs to happen before the function call, otherwise clients is moved into process_socket
            // and then we don't get it back for the next iteration of the loop
            let clients = Arc::clone(&clients);         
            tokio::spawn(process_socket(sock, clients))
        });

    tokio::run(server);
}

