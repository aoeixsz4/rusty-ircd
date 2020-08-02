extern crate tokio;
extern crate futures;
#[macro_use]
mod irc;
mod buffer;
mod client;
mod parser;

use std::sync::{Arc, Weak, Mutex};
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use crate::buffer::MessageBuffer;
use crate::client::{Client, ClientFuture};
use crate::irc::{Core, NamedEntity};
use futures::{task, Stream};

// will want to at some point merge this with existing client and messagebuffer code in client.rs
// and buffer.rs

fn process_socket(sock: TcpStream, irc_core: Core) -> ClientFuture {
    // borrow checker complains about mutex locked clients
    // borrowing stuff when a move happens
    // so we can deliberately descope it before that
    // scope id here to use later
    //
    // need a proper Arc copy in order to write to this
    // and not have borrowing/moving problems, I think
    // also remember to clone a reference
    let id_arc = Arc::clone(&irc_core.id_counter);
    let mut id = id_arc.write().unwrap();
    let client = {
        let client = Arc::new(Mutex::new(Client::new(*id, task::current(), sock)));
        // actual hashmap is inside ClientList struct
        let mut clients = irc_core.clients.write().unwrap();
        clients.insert(*id, Arc::downgrade(&client));

        // increment id value, this will only ever go up, integer overflow will wreak havoc,
        // but i doubt we reach enough clients for this to happen - should
        // be handled in any final release of code though, *just in case*
        println!("client connected: id = {}", *id);
        *id += 1;
        client
    }; // drop mutex locked clients list
    //
    // we can now return a future containing client data and a pointer to the client list,
    // and we don't have any reference cycles
    ClientFuture {
        client,
        // irc_core is already cloned in main()
        irc_core,        // client_list is now within irc_core
        id: *id,
        first_poll: true
    }
}
        

fn main() {
    let addr = "127.0.0.1:6667".parse::<SocketAddr>().unwrap();
    // Ups, this will just try to connect to above address,
    // we want to bind to it
    let listener = TcpListener::bind(&addr).unwrap();
    let irc_core = Core::new();

    let server = listener.incoming()
        .map_err(|e| println!("failed to accept stream, error = {:?}", e))
        .for_each(move |sock| {
            // clone needs to happen before the function call, otherwise clients is moved into process_socket
            // and then we don't get it back for the next iteration of the loop
            tokio::spawn(process_socket(sock, irc_core.clone()))
        });

    tokio::run(server);
}

