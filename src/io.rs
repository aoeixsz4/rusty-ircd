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
extern crate tokio;
extern crate tokio_native_tls;
use core::pin::Pin;
use core::result::Result;
use core::task::{Context, Poll};
use tokio::io::Error as tioError;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_native_tls::TlsStream;

/* implement AsyncRead/Write and AsyncRead/WriteExt on wrappers so that the
 * rest of our code need not care whether we're dealing with ClearText or
 * a TLS/SSL connection */
#[derive(Debug)]
pub enum ReadHalfWrap {
    ClearText(ReadHalf<TcpStream>),
    Encrypted(ReadHalf<TlsStream<TcpStream>>)
}

#[derive(Debug)]
pub enum WriteHalfWrap {
    ClearText(WriteHalf<TcpStream>),
    Encrypted(WriteHalf<TlsStream<TcpStream>>)
}

impl AsyncRead for ReadHalfWrap {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut ReadBuf) -> Poll<Result<(), tioError>> {
        let wrapper = Pin::into_inner(self);
        match wrapper {
            ReadHalfWrap::ClearText(inner) => AsyncRead::poll_read(Pin::new(inner), cx, buf),
            ReadHalfWrap::Encrypted(inner) => AsyncRead::poll_read(Pin::new(inner), cx, buf)
        }
    }
}

impl AsyncWrite for WriteHalfWrap {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, tioError>> {
        let wrapper = Pin::into_inner(self);
        match wrapper {
            WriteHalfWrap::ClearText(inner) => AsyncWrite::poll_write(Pin::new(inner), cx, buf),
            WriteHalfWrap::Encrypted(inner) => AsyncWrite::poll_write(Pin::new(inner), cx, buf)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), tioError>> {
        let wrapper = Pin::into_inner(self);
        match wrapper {
            WriteHalfWrap::ClearText(inner) => AsyncWrite::poll_flush(Pin::new(inner), cx),
            WriteHalfWrap::Encrypted(inner) => AsyncWrite::poll_flush(Pin::new(inner), cx)
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), tioError>> {
        let wrapper = Pin::into_inner(self);
        match wrapper {
            WriteHalfWrap::ClearText(inner) => AsyncWrite::poll_shutdown(Pin::new(inner), cx),
            WriteHalfWrap::Encrypted(inner) => AsyncWrite::poll_shutdown(Pin::new(inner), cx)
        }
    }
}
