/* rusty-ircd - an IRC daemon written in Rust
*  Copyright (C) Joanna Janet Zaitseva-Doyle <jjadoyle@gmail.com>

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

pub mod irc;
pub mod client;
pub mod parser;
use crate::client::{run_client_handler, run_write_task, Host};
use crate::irc::Core;
use dns_lookup::lookup_addr;
use std::io::Error as ioError;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task;

fn get_host(ip_addr: IpAddr) -> Result<Host, ioError> {
    match lookup_addr(&ip_addr) {
        Ok(h) => Ok(Host::Hostname(h)),
        Err(_) => Ok(Host::HostAddr(ip_addr)),
    }
}

async fn process_socket(sock: TcpStream, irc: Arc<Core>) -> Result<(), ioError> {
    let id = irc.assign_id();
    /* Two ? required, one expects a potential JoinError, the second ?
     * decomposes to give Host or an ioError - may need some additional error
     * composition to deal with the possible JoinError... */
    let ip_address = sock.peer_addr()?.ip();
    let host = task::spawn_blocking(move || get_host(ip_address)).await??;
    let (tx, rx) = mpsc::channel(32);
    let (read, write) = sock.into_split();
    tokio::spawn(run_write_task(write, rx));
    tokio::spawn(run_client_handler(id, host, irc, tx, read));
    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let mut listener = TcpListener::bind("127.0.1.1:6667").await?;
    let server_host = if let Ok(ip) = "127.0.1.1".parse::<IpAddr>() {
        if let Host::Hostname(h) = task::spawn_blocking(move ||get_host(ip)).await?? {
            h
        } else {
            "localhost".to_string()
        }
    } else {
        "localhost".to_string()
    };
    let irc_core = Core::new(server_host);
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(process_socket(socket, Arc::clone(&irc_core)));
    }
}
