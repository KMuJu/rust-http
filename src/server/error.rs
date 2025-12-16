use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Internal Error")]
    InternalError,
    #[error("IO: {0}")]
    IO(#[from] io::Error),

    #[error("TLS: {0}")]
    TLS(#[from] rustls::Error),

    #[error("Pem: {0}")]
    Pem(#[from] rustls::pki_types::pem::Error),
}
