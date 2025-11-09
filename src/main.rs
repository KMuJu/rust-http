use std::{
    io::{Result, Write},
    net::{TcpListener, TcpStream},
};

use crate::message::{request::RequestParser, response::Response, status_line::StatusCode};

mod message;

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:42069").expect("Could not bind to addr");

    for stream in listener.incoming() {
        handle_client(stream?);
    }
    Ok(())
}

fn handle_client(mut stream: TcpStream) {
    let request = RequestParser::request_from_reader(&mut stream);
    match request {
        Ok(req) => {
            // println!("Got request: {:?}", req);
            println!(
                "Got request for: {:?} {} {}",
                req.line.method, req.line.url, req.line.version
            );

            let mut resp = Response::new(StatusCode::Ok);
            if let Err(e) = resp.write_all(b"Hello World") {
                eprint!("Error when writing to body: {}", e);
            }
            if let Err(e) = resp.write_to(&mut stream) {
                eprint!("Error when writing to stream: {}", e);
            }
        }
        Err(e) => eprint!("Error: {}", e),
    }
    println!("Finished with client");
}
