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
use crate::buffer::MessageBuffer;
use crate::client::{Client, ClientFuture, ClientList};

// will want to at some point merge this with existing client and messagebuffer code in client.rs
// and buffer.rs

fn process_socket(sock: TcpStream, clients: Arc<Mutex<ClientList>>) -> ClientFuture {
    // borrow checker complains about mutex locked clients
    // borrowing stuff when a move happens
    // so we can deliberately descope it before that
    // scope id here to use later
    let mut id = 0;
    let client = {
        let mut clients = clients.lock().unwrap();
        id = clients.next_id;
        let client = Arc::new(Mutex::new(Client::new(id, task::current, sock)));
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

