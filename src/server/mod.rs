mod error;
mod threadpool;

pub use error::ServerError;

use std::net::{TcpListener, TcpStream};

use crate::{
    message::{
        Response, ResponseBuilder, StatusCode, {Request, RequestParser},
    },
    server::threadpool::ThreadPool,
};

pub struct Server {
    pool: ThreadPool,
    handler: Handler,
    _addr: String,
    listener: TcpListener,
}

type Handler = fn(&Request) -> Result<Response, ServerError>;

impl Server {
    pub fn new(addr: &str, handler: Handler) -> Server {
        let pool = ThreadPool::new(8);
        let listener = TcpListener::bind(addr).expect("Could not bind to addr: {addr}");
        Server {
            pool,
            handler,
            _addr: addr.to_string(),
            listener,
        }
    }

    pub fn listen_and_serve(&self) {
        let handler = self.handler;
        for stream in self.listener.incoming() {
            let stream = stream.unwrap();
            self.pool.execute(move || {
                handle_connection(stream, handler);
            });
        }
    }
}

fn internal_error(mut stream: TcpStream) {
    let mut builder = ResponseBuilder::new();
    builder.set_status_code(StatusCode::InternalServerError);
    let mut response = builder.build();
    let r = response.write_to(&mut stream);
    assert!(r.is_ok(), "Failed to write internal error to tcp stream");
    // Something is wrong if it can't write to the stream
}

    fn handle_connection(mut stream: TcpStream, handler: Handler) {
        let request = RequestParser::request_from_reader(&mut stream);

        let Ok(request) = request else {
            return Server::internal_error(stream);
        };

        let response = handler(&request);

        let Ok(mut response) = response else {
            return Server::internal_error(stream);
        };

        if response.write_to(&mut stream).is_err() {
            Server::internal_error(stream);
        }
    }
}
