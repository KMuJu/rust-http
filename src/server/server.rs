use std::net::{TcpListener, TcpStream};

use crate::{
    message::{
        request::{Request, RequestParser},
        response::Response,
        status_line::StatusCode,
    },
    server::{error::ServerError, response_builder::ResponseBuilder, threadpool::ThreadPool},
};

pub struct Server {
    pool: ThreadPool,
    handler: Handler,
    addr: String,
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
            addr: addr.to_string(),
            listener,
        }
    }

    pub fn listen_and_serve(&self) {
        let handler = self.handler;
        for stream in self.listener.incoming() {
            let stream = stream.unwrap();
            self.pool.execute(move || {
                Server::handle_connection(stream, handler);
            });
        }
    }

    fn internal_error(mut stream: TcpStream) {
        let mut builder = ResponseBuilder::new();
        builder.set_status_code(StatusCode::InternalServerError);
        let mut response = builder.build();
        response.write_to(&mut stream);
    }

    fn handle_connection(mut stream: TcpStream, handler: Handler) {
        let request = RequestParser::request_from_reader(&mut stream);
        // match request {
        //     Ok(req) => {
        //         let response = handler(&req);
        //     }
        //     Err(e) => {}
        // };

        let Ok(request) = request else {
            return Server::internal_error(stream);
        };

        let response = handler(&request);

        let Ok(mut response) = response else {
            return Server::internal_error(stream);
        };

        if response.write_to(&mut stream).is_err() {
            return Server::internal_error(stream);
        }
    }
}
