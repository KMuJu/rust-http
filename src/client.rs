use std::net::SocketAddr;

use tokio::net::TcpSocket;

use crate::{
    client::error::ClientError,
    message::{Request, Response, ResponseParser},
};
pub mod error;

pub async fn send_request(url: &str, req: &mut Request) -> Result<Response, ClientError> {
    let addr = tokio::net::lookup_host(format!("{url}:80"))
        .await?
        .next()
        .ok_or(ClientError::UrlNotFound)?;

    let socket = match addr {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };

    println!("Addr: {addr:?}");

    println!("Req: {req:?}");

    let mut stream = socket.connect(addr).await?;

    req.write_to(&mut stream).await?;

    println!("Wrote request to stream");

    let resp = ResponseParser::response_from_reader(&mut stream).await?;

    Ok(resp)
}
