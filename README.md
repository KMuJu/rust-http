# Simple http 1.1 server

Uses a tokio async to handle incomming requests.
Version with threadpool can be found in old_with_threads branch.

## Usage

```rust
#[tokio::main]
async fn main() {
    let server = Server::new("localhost:42069", handle_request).await;
    let r = server.listen_and_serve().await;
    if let Err(e) = r {
        eprint!("Error while listening: {e}")
    }
}

// This function is called for every request
fn handle_request(req: &Request) -> Result<Response, ServerError> {
    match (req.get_method(), req.get_url()) {
        (Method::Get, "/") => {
            // Can use a response builder to create the response
            let mut builder = ResponseBuilder::new();
            builder.add_to_body(b"Hello World")?;
            // Response has by default status 200 Ok
            Ok(builder.build())
        }
        (_, _) => {
            let mut builder = ResponseBuilder::new();
            builder.set_status_code(StatusCode::NotFound);
            Ok(builder.build())
        }
    }
}
```

Examples can be found in [examples](./examples/)

## Supports

- Chunked encoding
- Keep-alive connections
- Parsing requests and sending responses

## Not supported

- Trailers
- Other transfer encodings other than chunked
- Streaming responses
- Routing

## Future additions

- HTTP client to be able to send requests

## Structure

This project has two parts:
- Messages
- Server

The messages module is for parsing and creating messages, while the server module is for handeling requests.

### Server

The server uses `tokio::spawn` for each connection and will call the handler function for each request.
The task will run until the connection closes.
The spawned task is really simple.
It parses the request, calls the handler, sends the response.
It sends an internal error response whenever there is an error during the task.

### Messages

In the message module, there is all the parts making up requests and responses, as well as parsers.
The parsers tries their best to follow RFC 9112 and 9110.
They are implemented as statemachines that goes through the parts needed to create a request or response.
They will remain in the same state until something indicates that it needs to go to the next state.
This allows the parser able to read requests that are split into multiple read calls.

## Development

Since there are clear rules as to what should happen thanks to the RFCs, this project was developed with a lot of test driven development.
