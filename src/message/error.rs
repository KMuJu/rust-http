use std::io::Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RequestLineError {
    #[error("Malformed request line")]
    MalformedRequestLine,

    #[error("Invalid method")]
    InvalidMehtod,
}

#[derive(Debug, Error)]
pub enum HeadersError {
    #[error("Malformed header")]
    MalformedFieldLine,

    #[error("Contained both Transfer-Encoding and Content-Length")]
    InvalidHeaderFields,

    #[error("Invalid Content-Length value")]
    InvalidContentLength,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("Malformed request line: {0}")]
    RequestLine(#[from] RequestLineError),

    #[error("Malformed header: {0}")]
    Header(#[from] HeadersError),

    #[error("Malformed request")]
    MalformedRequest,

    #[error("Body longer than content-length")]
    BodyTooLong,

    #[error("Malformed chunked body")]
    MalformedChunkedBody,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("IO error: {0}")]
    IO(#[from] Error),
}
