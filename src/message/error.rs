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
    MalformedHeader,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("Malformed request line: {0}")]
    MalformedRequestLine(#[from] RequestLineError),

    #[error("Malformed header: {0}")]
    MalformedHeader(#[from] HeadersError),

    #[error("Malformed request")]
    MalformedRequest,

    #[error("Invalid content-length")]
    InvalidContentLength,

    #[error("Body longer than content-length")]
    BodyTooLong,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("IO error: {0}")]
    IO(#[from] Error),
}
