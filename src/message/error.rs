use std::io::Error;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VersionError {
    #[error("Invalid http version")]
    InvalidHTTPVersion,
}

#[derive(Debug, Error)]
pub enum RequestLineError {
    #[error("Malformed request line")]
    MalformedRequestLine,

    #[error("Invalid method")]
    InvalidMehtod,

    #[error("Invalid http version")]
    InvalidHTTPVersion(#[from] VersionError),
}

#[derive(Debug, Error)]
pub enum StatusLineError {
    #[error("Malformed status line")]
    MalformedStatusLine,

    #[error("Invalid method")]
    InvalidStatusCode,

    #[error("Invalid http version")]
    InvalidHTTPVersion(#[from] VersionError),
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

    #[error("Malformed body: {0}")]
    Body(#[from] BodyError),

    #[error("Malformed request")]
    MalformedRequest,

    #[error("Body longer than content-length")]
    BodyTooLong,

    #[error("Malformed chunked size")]
    MalformedChunkedSize,

    #[error("Malformed chunked body")]
    MalformedChunkedBody,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("Malformed status line: {0}")]
    StatusLine(#[from] StatusLineError),

    #[error("Malformed header: {0}")]
    Header(#[from] HeadersError),

    #[error("Malformed body: {0}")]
    Body(#[from] BodyError),

    #[error("Malformed response")]
    MalformedResponse,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}

#[derive(Debug, Error)]
pub enum BodyError {
    #[error("Malformed header: {0}")]
    Header(#[from] HeadersError),

    #[error("Body longer than content-length")]
    TooLong,

    #[error("Malformed chunked size")]
    MalformedChunkedSize,

    #[error("Malformed chunked body")]
    MalformedChunkedBody,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}

#[derive(Debug, Error)]
pub enum StreamError {
    #[error("Unexpected EOF")]
    EOF,

    #[error("IO error: {0}")]
    IO(#[from] Error),
}
