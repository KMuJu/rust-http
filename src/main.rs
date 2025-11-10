use std::io;

use rust_http::message::method::Method;
use rust_http::message::status_line::StatusCode;
use rust_http::server::{error::ServerError, response_builder::ResponseBuilder, server::Server};

use rust_http::message::{request::Request, response::Response};

fn main() {
    let server = Server::new("localhost:42069", handle_request);
    server.listen_and_serve();
}

fn handle_request(req: &Request) -> Result<Response, ServerError> {
    println!("Got request to: {:?} {}", req.line.method, req.line.url);
    match (req.get_method(), req.get_url()) {
        (Method::Get, "/") => {
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"Hello World")?;
            Ok(builder.build())
        }
        (_, _) => {
            let mut builder = ResponseBuilder::new();
            builder.set_status_code(StatusCode::NotFound);
            Ok(builder.build())
        }
    }
}
