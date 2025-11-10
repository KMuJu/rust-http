use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Internal Error")]
    InternalError,
    #[error("IO: {0}")]
    IO(#[from] io::Error),
}
