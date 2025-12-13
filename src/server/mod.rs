mod error;

use std::io;

pub use error::ServerError;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::message::{Connection, Request, RequestError, Response, ResponseBuilder, StatusCode};

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
            let (mut stream, _) = self.listener.accept().await?;
            let addr = stream.peer_addr().unwrap();
            println!("Got request from: {:?}", addr);

            tokio::spawn(async move {
                let (r, w) = stream.split();
                let connection = Connection::<_, _, Request>::new(r, w);
                handle_connection(connection, handler).await;
                println!("Closing connection");
            });
        }
    }
}

async fn internal_error<R, W>(connection: &mut Connection<R, W, Request>)
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut builder = ResponseBuilder::new();
    builder.set_status_code(StatusCode::InternalServerError);
    let mut response = builder.build();
    let r = connection.respond(&mut response).await;
    if let Err(e) = r {
        eprintln!("Failed to write internal error to tcp stream");
        eprintln!("{e}");
        // Something is wrong if it can't write to the stream
    }
}

/// Tries to read request
/// Then passes it to the handler
/// Then writes the returning response to the stream
///
/// If any of the above failes, it will write an InternalServerError response to the stream
async fn handle_connection<R, W>(mut connection: Connection<R, W, Request>, handler: Handler)
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    loop {
        let request = connection.read().await;

        let request = match request {
            Ok(req) => req,
            Err(RequestError::IO(e))
                if e.kind() == io::ErrorKind::UnexpectedEof
                    || e.kind() == io::ErrorKind::ConnectionAborted
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                eprintln!("IO error handling request: {e}");
                break;
            }
            Err(_) => {
                internal_error(&mut connection).await;
                break;
            }
        };

        let response = handler(&request);

        let mut response = match response {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Error handling request: {e:?}");
                internal_error(&mut connection).await;
                break;
            }
        };

        if connection.respond(&mut response).await.is_err() {
            internal_error(&mut connection).await;
            break;
        }

        if should_close(&request, &response) {
            break;
        }
    }
}

fn should_close(req: &Request, resp: &Response) -> bool {
    if req.line.version == (1, 0) && !req.headers.field_contains_value("Connection", "keep-alive") {
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

        let input = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec();
        let fake_stream = Cursor::new(input.clone());
        let mut v = Cursor::new(Vec::new());
        let connection = Connection::<_, _, Request>::new(fake_stream, &mut v);

        fn test_handler(_: &Request) -> Result<Response, ServerError> {
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"ok").unwrap();
            Ok(builder.build())
        }

        handle_connection(connection, test_handler).await;

        let written = v.into_inner();
        assert!(String::from_utf8_lossy(&written).contains("ok"));
    }

    #[tokio::test]
    async fn test_server_handles_request() {
        let server = Server::test(fake_handler).await;
        let addr = server.listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = server.listener.accept().await {
                let (r, w) = stream.split();
                let connection = Connection::<_, _, Request>::new(r, w);
                handle_connection(connection, server.handler).await;
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
            if let Ok((mut stream, _)) = server.listener.accept().await {
                let (r, w) = stream.split();
                let connection = Connection::<_, _, Request>::new(r, w);
                handle_connection(connection, server.handler).await;
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
