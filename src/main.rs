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
    input: Arc<Mutex<MessageBuffer>>,
    output: Arc<Mutex<MessageBuffer>>,
    handler: Task,
    id: u32
}

struct ClientFuture {
    client: Arc<Mutex<Client>>,
    client_list: Arc<Mutex<ClientList>>,
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
        let client = Arc::clone(&self.client);
        let mut client = client.lock().unwrap();  // client is now under mutex lock
        // try to write if there is anything in outbuf,
        // bad idea to loop here, unless perhaps we have a break statement on Async::NotReady?
        // yeah, that should be fairly safe, then we at least try to empty the buffer until we
        // can't write anymore

        // now we also have the slightly annoying situation that if bytes_out < out_i,
        // we have to either do someething complicated with two indices, or shift
        // bytes to the start of the buffer every time a write completes,
        // other option would be to have an extra index only within this loop,
        // and shift bytes after a break; if absolutely necessary
        // other option: keep beginning and end indices in the Client struct,
        // try to shift everything to the start of the buffer only if a new append would
        // overrun it
        let mut write_count: usize = 0;
        let mut tmp_buf: [u8; rfc::MAX_MSG_SIZE] = [0; rfc::MAX_MSG_SIZE];

        // create a special scope where we use the Arc<Mutex<>> wrapper to copy stuff into 
        // our temporary write buffer
        let len = {
            let outbuf = Arc::clone(&client.output);
            let outbuf = outbuf.lock().unwrap();
            if outbuf.index > 0 {
                outbuf.copy(&mut tmp_buf) // returns bytes copied
            } else {
                0
            }
        };

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
        } // mutex dropped here

                    
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
                    
                    // need to notify
                    other_client.handler.notify();
                }
            }
        }
        Ok(Async::NotReady)
    }
}

fn process_socket(sock: TcpStream, clients: Arc<Mutex<ClientList>>) -> ClientFuture {
    // borrow checker complains about mutex locked clients
    // borrowing stuff when a move happens
    // so we can deliberately descope it before that
    let client = {
        let mut clients = clients.lock().unwrap();
        let id = clients.next_id;
        let client = Arc::new(Mutex::new(Client {
            socket: sock,
            input: Arc::new(Mutex::new(MessageBuffer::new())),
            output: Arc::new(Mutex::new(MessageBuffer::new())),
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
        client_list: clients
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

