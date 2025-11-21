use std::io;

use thiserror::Error;

use crate::message::ResponseError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Internal Error")]
    InternalError,

    #[error("Url not found")]
    UrlNotFound,

    #[error("Response error: {0}")]
    ResponseError(#[from] ResponseError),

    #[error("IO: {0}")]
    IO(#[from] io::Error),
}
