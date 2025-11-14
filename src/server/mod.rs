mod error;
mod threadpool;

pub use error::ServerError;
pub use threadpool::ThreadPool;

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use crate::{
    message::{Request, RequestParser, Response, ResponseBuilder, StatusCode},
    server::threadpool::Executor,
};

pub trait Stream: Read + Write + Send {}
impl<T: Read + Write + Send> Stream for T {}

/// HTTP Server
///
/// Uses a threadpool to handle requests
///
pub struct Server<E: Executor> {
    pool: E,
    handler: Handler,
    addr: String,
    listener: TcpListener,
}

type Handler = fn(&Request) -> Result<Response, ServerError>;

impl Server<ThreadPool> {
    pub fn new(addr: &str, handler: Handler) -> Server<ThreadPool> {
        let pool = ThreadPool::new(8);
        let listener = TcpListener::bind(addr).expect("Could not bind to addr: {addr}");
        Server {
            pool,
            handler,
            addr: addr.to_string(),
            listener,
        }
    }

    /// Listens to incoming streams, sending them to the threadpool
    ///
    /// # Panics
    ///
    /// Panics if it can't send the job to the threadpool
    pub fn listen_and_serve(&self) {
        println!("Listening to: {:?}", self.addr);
        let handler = self.handler;
        for stream in self.listener.incoming() {
            let stream = stream.unwrap();
            self.pool.execute(move || {
                handle_connection(stream, handler);
            });
        }
    }
}

fn internal_error<S: Stream>(stream: &mut S) {
    let mut builder = ResponseBuilder::new();
    builder.set_status_code(StatusCode::InternalServerError);
    let mut response = builder.build();
    let r = response.write_to(stream);
    assert!(r.is_ok(), "Failed to write internal error to tcp stream");
    // Something is wrong if it can't write to the stream
}

/// Tries to read request
/// Then passes it to the handler
/// Then writes the returning response to the stream
///
/// If any of the above failes, it will write an InternalServerError response to the stream
fn handle_connection<S: Stream>(mut stream: S, handler: Handler) {
    let request = RequestParser::request_from_reader(&mut stream);

    let Ok(request) = request else {
        internal_error(&mut stream);
        return;
    };

    let response = handler(&request);

    let Ok(mut response) = response else {
        internal_error(&mut stream);
        return;
    };

    if response.write_to(&mut stream).is_err() {
        internal_error(&mut stream);
    }

    // TODO: Handle keep alive connection
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use std::thread;

    use super::*;

    impl Executor for FakeExecutor {
        fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send + 'static,
        {
            f();
        }
    }

    fn fake_handler(_: &Request) -> Result<Response, ServerError> {
        let mut builder = ResponseBuilder::new();
        builder.add_to_body(b"Hello")?;
        Ok(builder.build())
    }

    #[derive(Clone)]
    struct FakeExecutor;
    impl Server<FakeExecutor> {
        pub fn test(handler: Handler) -> Server<FakeExecutor> {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            Server {
                pool: FakeExecutor,
                handler,
                addr: "".to_string(),
                listener,
            }
        }
    }

    #[test]
    fn test_handle_connection_ok() {
        use std::io::Cursor;

        let mut fake_stream = Cursor::new(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n".to_vec());

        fn test_handler(_: &Request) -> Result<Response, ServerError> {
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"ok").unwrap();
            Ok(builder.build())
        }

        handle_connection(&mut fake_stream, test_handler);

        let written = fake_stream.into_inner();
        assert!(String::from_utf8_lossy(&written).contains("ok"));
    }

    #[test]
    fn test_server_handles_request() {
        let server = Server::test(fake_handler);
        let addr = server.listener.local_addr().unwrap();

        thread::spawn(move || {
            if let Ok((stream, _)) = server.listener.accept() {
                handle_connection(stream, server.handler);
            }
        });

        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap();
        let mut builder = ResponseBuilder::new();
        builder.add_to_body(b"Hello").unwrap();
        let mut response = builder.build();

        let mut expected = Vec::new();
        response.write_to(&mut expected).unwrap();
        let output = String::from_utf8_lossy(&buf);
        let expected = String::from_utf8_lossy(&expected);

        assert_eq!(output, expected,);
    }
}
