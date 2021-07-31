/* rusty-ircd - an IRC daemon written in Rust
*  Copyright (C) 2020 Joanna Janet Zaitseva-Doyle <jjadoyle@gmail.com>

*  This program is free software: you can redistribute it and/or modify
*  it under the terms of the GNU Lesser General Public License as
*  published by the Free Software Foundation, either version 3 of the
*  License, or (at your option) any later version.

*  This program is distributed in the hope that it will be useful,
*  but WITHOUT ANY WARRANTY; without even the implied warranty of
*  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
*  GNU Lesser General Public License for more details.

*  You should have received a copy of the GNU Lesser General Public License
*  along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
extern crate dns_lookup;
extern crate log;
extern crate tokio;
extern crate tokio_native_tls;
pub mod irc;
pub mod client;
pub mod io;
use crate::client::{run_client_handler, run_write_task, Host, GenError};
use crate::io::{ReadHalfWrap, WriteHalfWrap};
use crate::irc::Core;
use dns_lookup::lookup_addr;
use std::fs::File;
use std::io::Error as ioError;
use std::io::Read;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::io::split;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task;
use tokio_native_tls::TlsAcceptor;
use tokio_native_tls::native_tls::Identity;
use tokio_native_tls::native_tls::TlsAcceptor as NativeTlsAcc;

pub const USER_MODES: &str = "";
pub const CHAN_MODES: &str = "+o";

fn get_host(ip_addr: IpAddr) -> Result<Host, ioError> {
    match lookup_addr(&ip_addr) {
        Ok(h) => Ok(Host::Hostname(h)),
        Err(_) => Ok(Host::HostAddr(ip_addr)),
    }
}

async fn plaintext_socket(sock: TcpStream, irc: Arc<Core>) -> Result<(), GenError> {
    let id = irc.assign_id();
    /* Two ? required, one expects a potential JoinError, the second ?
     * decomposes to give Host or an ioError - may need some additional error
     * composition to deal with the possible JoinError... */
    let ip_address = sock.peer_addr()?.ip();
    let host = task::spawn_blocking(move || get_host(ip_address)).await??;
    let (tx, rx) = mpsc::channel(32);
    let (read, write) = split(sock);
    tokio::spawn(run_write_task(WriteHalfWrap::ClearText(write), rx));
    tokio::spawn(run_client_handler(
        id,
        host,
        irc,
        tx,
        ReadHalfWrap::ClearText(read),
    ));
    Ok(())
}

async fn plain_listen(server: TcpListener, irc_core: Arc<Core>) -> Result<(), GenError> {
    loop {
        let (socket, _) = server.accept().await?;
        tokio::spawn(plaintext_socket(socket, Arc::clone(&irc_core)));
    }
}

async fn process_socket(sock: TcpStream, irc: Arc<Core>, acceptor: Arc<TlsAcceptor>) -> Result<(), GenError> {
    let id = irc.assign_id();
    /* Two ? required, one expects a potential JoinError, the second ?
     * decomposes to give Host or an ioError - may need some additional error
     * composition to deal with the possible JoinError... */
    let ip_address = sock.peer_addr()?.ip();
    let host = task::spawn_blocking(move || get_host(ip_address)).await??;
    let (tx, rx) = mpsc::channel(32);
    let tls_stream = acceptor.accept(sock).await?;
    let (read, write) = split(tls_stream);
    tokio::spawn(run_write_task(WriteHalfWrap::Encrypted(write), rx));
    tokio::spawn(run_client_handler(
        id,
        host,
        irc,
        tx,
        ReadHalfWrap::Encrypted(read),
    ));
    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version = env!("CARGO_PKG_NAME").to_string() + ", version: " + env!("CARGO_PKG_VERSION");
    env_logger::init();

    // is this even necessary?
    let server_host = if let Ok(ip) = "127.0.1.1".parse::<IpAddr>() {
        if let Host::Hostname(h) = task::spawn_blocking(move ||get_host(ip)).await?? {
            h
        } else {
            "localhost".to_string()
        }
    } else {
        "localhost".to_string()
    };
    let irc_core = Core::new(server_host, version);

    // encryption key stuff
    let mut file = File::open("identity.pfx").unwrap();
    let mut identity = vec![];
    file.read_to_end(&mut identity).unwrap();
    let identity = Identity::from_pkcs12(&identity, "password").expect("failed to get identity, check password?");

    // start raw socket listeners
    let plain_listener = TcpListener::bind("127.0.1.1:6667").await?;
    let listener = TcpListener::bind("127.0.1.1:6697").await?;
    
    // spawn routine to deal with plaintext clients
    tokio::spawn(plain_listen(plain_listener, Arc::clone(&irc_core)));

    // first create the non-async TlsAcceptor
    let acceptor = NativeTlsAcc::new(identity).unwrap();

    // this creates the tokio wrapper
    let acceptor = Arc::new(TlsAcceptor::from(acceptor));
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(process_socket(socket, Arc::clone(&irc_core), Arc::clone(&acceptor)));
    }
}
