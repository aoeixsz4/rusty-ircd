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
