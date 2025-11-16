mod error;

pub use error::ServerError;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpListener;

use crate::message::{Request, RequestError, RequestParser, Response, ResponseBuilder, StatusCode};

pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// HTTP Server
///
/// Uses a threadpool to handle requests
///
pub struct Server {
    handler: Handler,
    _addr: String,
    listener: TcpListener,
}

type Handler = fn(&Request) -> Result<Response, ServerError>;

impl Server {
    pub async fn new(addr: &str, handler: Handler) -> Server {
        let listener = TcpListener::bind(addr)
            .await
            .expect("Could not bind to addr: {addr}");
        Server {
            handler,
            _addr: addr.to_string(),
            listener,
        }
    }

    /// Listens to incoming streams, sending them to the threadpool
    ///
    /// # Panics
    ///
    /// Panics if it can't send the job to the threadpool
    pub async fn listen_and_serve(&self) -> Result<(), ServerError> {
        let addr = self.listener.local_addr().unwrap();
        println!("Listening to: {:?}", addr);
        let handler = self.handler;

        loop {
            let (stream, _) = self.listener.accept().await?;
            let addr = stream.peer_addr().unwrap();
            println!("Got request from: {:?}", addr);

            tokio::spawn(async move {
                handle_connection(stream, handler).await;
            });
        }
    }
}

async fn internal_error<S: Stream>(stream: &mut S) {
    let mut builder = ResponseBuilder::new();
    builder.set_status_code(StatusCode::InternalServerError);
    let mut response = builder.build();
    let r = response.write_to(stream).await;
    if r.is_err() {
        eprintln!("Failed to write internal error to tcp stream");
        // Something is wrong if it can't write to the stream
    }
}

/// Tries to read request
/// Then passes it to the handler
/// Then writes the returning response to the stream
///
/// If any of the above failes, it will write an InternalServerError response to the stream
async fn handle_connection<S: Stream>(mut stream: S, handler: Handler) {
    loop {
        let request = RequestParser::request_from_reader(&mut stream).await;

        let request = match request {
            Ok(req) => req,
            Err(RequestError::MalformedRequest) => {
                eprintln!("Got EOF while parsing");
                break;
            }
            Err(_) => {
                internal_error(&mut stream).await;
                break;
            }
        };

        let response = handler(&request);

        let mut response = match response {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Error handling request: {e:?}");
                internal_error(&mut stream).await;
                break;
            }
        };

        if response.write_to(&mut stream).await.is_err() {
            internal_error(&mut stream).await;
            break;
        }

        if should_close(&request, &response) {
            break;
        }
    }
    println!("Closing connection");
}

fn should_close(req: &Request, resp: &Response) -> bool {
    if req.line.version == "1.0" && !req.headers.field_contains_value("Connection", "keep-alive") {
        return true;
    }
    if req.headers.field_contains_value("Connection", "close") {
        return true;
    }
    if resp.headers.field_contains_value("Connection", "close") {
        return true;
    }
    false
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    use super::*;

    fn fake_handler(_: &Request) -> Result<Response, ServerError> {
        let mut builder = ResponseBuilder::new();
        builder.add_to_body(b"Hello")?;
        Ok(builder.build())
    }
    fn fake_handler_no_body(_: &Request) -> Result<Response, ServerError> {
        let builder = ResponseBuilder::new();
        Ok(builder.build())
    }

    impl Server {
        pub async fn test(handler: Handler) -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            Server {
                handler,
                _addr: "".to_string(),
                listener,
            }
        }
    }

    #[tokio::test]
    async fn test_handle_connection_ok() {
        use std::io::Cursor;

        let mut fake_stream = Cursor::new(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec());

        fn test_handler(_: &Request) -> Result<Response, ServerError> {
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"ok").unwrap();
            Ok(builder.build())
        }

        handle_connection(&mut fake_stream, test_handler).await;

        let written = fake_stream.into_inner();
        assert!(String::from_utf8_lossy(&written).contains("ok"));
    }

    #[tokio::test]
    async fn test_server_handles_request() {
        let server = Server::test(fake_handler).await;
        let addr = server.listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((stream, _)) = server.listener.accept().await {
                handle_connection(stream, server.handler).await;
            }
        });

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();

        let mut builder = ResponseBuilder::new();
        builder.add_to_body(b"Hello").unwrap();
        let mut response = builder.build();

        let mut expected = Vec::new();
        response.write_to(&mut expected).await.unwrap();
        let output = String::from_utf8_lossy(&buf);
        let expected = String::from_utf8_lossy(&expected);

        assert_eq!(output, expected,);
    }

    async fn read_one_response(stream: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 512];

        loop {
            let n = stream.read(&mut tmp).await.unwrap();
            if n == 0 {
                break; // server closed (unexpected for keep-alive)
            }
            buf.extend_from_slice(&tmp[..n]);

            // crude but works: detect full HTTP response by CRLF CRLF
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }

        String::from_utf8_lossy(&buf).to_string()
    }

    #[tokio::test]
    async fn test_server_handles_keep_alive() {
        let server = Server::test(fake_handler_no_body).await;
        let addr = server.listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((stream, _)) = server.listener.accept().await {
                handle_connection(stream, server.handler).await;
            }
        });

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();

        let resp1 = read_one_response(&mut stream).await;

        let mut response = ResponseBuilder::new().build();

        let mut expected = Vec::new();
        response.write_to(&mut expected).await.unwrap();
        let expected = String::from_utf8_lossy(&expected);

        assert_eq!(resp1, expected);

        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();

        let resp2 = read_one_response(&mut stream).await;

        assert_eq!(resp2, expected);
    }
}
