use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
use tokio_rustls::TlsStream;

pub enum Stream {
    Http(TcpStream),
    Https(TlsStream<TcpStream>),
}

impl From<TcpStream> for Stream {
    fn from(value: TcpStream) -> Self {
        Self::Http(value)
    }
}

impl From<TlsStream<TcpStream>> for Stream {
    fn from(value: TlsStream<TcpStream>) -> Self {
        Self::Https(value)
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Http(s) => Pin::new(s).poll_read(cx, buf),
            Stream::Https(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Stream::Http(s) => Pin::new(s).poll_write(cx, data),
            Stream::Https(s) => Pin::new(s).poll_write(cx, data),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Http(s) => Pin::new(s).poll_flush(cx),
            Stream::Https(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Stream::Http(s) => Pin::new(s).poll_shutdown(cx),
            Stream::Https(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
