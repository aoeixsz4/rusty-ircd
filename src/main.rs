extern crate tokio;
extern crate futures;
#[macro_use]

use std::sync::{Mutex, Arc};
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncRead, AsyncWrite};
use futures::{Future, Async, Poll, Stream};

struct Client {
    socket: TcpStream,
    // judging by the compiler errors, i think I will also need to wrap these fuckers in
    // Arc<Mutex<>>
    inbuf: [u8; 512],
    in_i: usize,
    outbuf: [u8; 512],
    out_i: usize,
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
        while client.out_i - write_count > 0 {
            match client.socket.poll_write(&(client.outbuf[write_count .. client.out_i])) {
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

        // update out_i depending on whether we wrote everything or broke out
        // nothing happens here if we had nothing to write in the first place
        if write_count == client.out_i {
            client.out_i = 0; // don't need to shift anything, we can just overwrite
        } else if write_count > 0 {
            for (i, j) in (write_count .. client.out_i).enumerate() {
                client.outbuf[i] = client.outbuf[j];
            }
            client.out_i -= write_count;
        }
                    
        while client.in_i < 512 { // loop until there's nothing to read or the buffer's full
            match client.socket.poll_read(&mut client.inbuf[client.in_i ..]) {
                Ok(Async::Ready(bytes_read)) => {
                    println!("Client wrote us!");
                    client.in_i += bytes_read;
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    println!("connection error: {}", e);
                    // remove client from our map...
                    // need to aquire a mutex lock
                    let mut client_list = self.client_list.lock().unwrap();
                    // we already have a lock on our client in the outer scope
                    client_list.map.remove(&client.id);  // oh oh. now the ID values in all clients after ours are incorrect
                                                    // maybe there's a better way to do this, and not
                                                    // have to update the id of every client each time?
                                                    // e.g. a hash map
                    return Ok(Async::Ready(())) // only call this on error - this Future completes when the client is no more
                }
            }
        }
        // if we end up here it's because we filled the buffer too much,
        // maybe just for this testing code we'll use that as a condition
        // for returning Async::Ready() and closing the connection
        let mut client_list = self.client_list.lock().unwrap();
        client_list.map.remove(&client.id);
        println!("buffer overflow on client {}! dropping connection", client.id);
        Ok(Async::Ready(()))
    }
}

fn process_socket(sock: TcpStream, clients: Arc<Mutex<ClientList>>) -> ClientFuture {
    // borrow checker complains about mutex locked clients
    // borrowing stuff when a move happens
    // so we can deliberately descope it before that
    let client = {
        let mut clients = clients.lock().unwrap();
        let client = Arc::new(Mutex::new(Client {
            socket: sock,
            outbuf: [0; 512],
            out_i: 0,
            inbuf: [0; 512],
            in_i: 0,
            id: clients.next_id
        }));
        // actual hashmap is inside ClientList struct
        clients.map.insert(clients.next_id, Arc::clone(&client));

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

