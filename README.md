# Simple http 1.1 server

> [!NOTE]
> This branch is old and from before I moved to tokio with async instead of threadpools
> This means that it does not have the new stuff :)

Uses a threadpool to handle incomming requests

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
