# Simple http 1.1 server

Uses a tokio async to handle incomming requests.
Version with threadpool can be found in old_with_threads branch.

## Usage

Examples can be found in [examples](./examples/)

## Not supported

- Trailers
- Other transfer encodings other than chunked
- Streaming responses
- Routing

## Future additions

- Async runtime such as tokio, instead of a threadpool
  This will decrease time waiting for requests.
  Since the server now accepts keep-alive connections,
  the server can currently only #threads alive connections
- HTTP client to be able to send requests
