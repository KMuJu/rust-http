use rust_http::message::Method;
use rust_http::message::ResponseBuilder;
use rust_http::message::StatusCode;
use rust_http::server::{Server, ServerError};

use rust_http::message::{Request, Response};

fn main() {
    let server = Server::new("localhost:42069", handle_request, 12);
    server.listen_and_serve();
}

fn handle_request(req: &Request) -> Result<Response, ServerError> {
    println!("Got request to: {:?} {}", req.line.method, req.line.url);
    match (req.get_method(), req.get_url()) {
        (Method::Get, "/") => {
            let resp = Response::from_file("examples/simple.html", "text/html; charset=utf-8")?;
            Ok(resp)
        }
        (Method::Get, "/hello") => {
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"Hello World")?;
            Ok(builder.build())
        }
        (Method::Post, "/upload") => {
            println!(
                "Uploaded body: {}",
                String::from_utf8_lossy(req.get_body()).escape_debug()
            );
            let mut builder = ResponseBuilder::new();
            builder.add_header("Connection", "close");
            Ok(builder.build())
        }
        (_, _) => {
            let mut builder = ResponseBuilder::new();
            builder.set_status_code(StatusCode::NotFound);
            Ok(builder.build())
        }
    }
}
