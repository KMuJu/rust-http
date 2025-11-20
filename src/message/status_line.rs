use std::io::{Result, Write};

use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Ok,                  // 200
    BadRequest,          // 400
    NotFound,            // 404
    MethodNotAllowed,    // 405
    InternalServerError, // 500
}

impl StatusCode {
    pub fn to_code(&self) -> String {
        match self {
            Self::Ok => "200",
            Self::BadRequest => "400",
            Self::NotFound => "404",
            Self::MethodNotAllowed => "405",
            Self::InternalServerError => "500",
        }
        .to_string()
    }
    pub fn to_reason(&self) -> String {
        match self {
            Self::Ok => "Ok",
            Self::BadRequest => "Bad Request",
            Self::NotFound => "Not Found",
            Self::MethodNotAllowed => "Method Not Allowed",
            Self::InternalServerError => "Internal Server Error",
        }
        .to_string()
    }
}

#[derive(Debug)]
pub struct StatusLine {
    pub version: String,
    pub status_code: StatusCode,
}

impl StatusLine {
    pub fn new(status_code: StatusCode) -> StatusLine {
        StatusLine {
            version: "1.1".to_string(),
            status_code,
        }
    }

    /// Follows RFC 9112
    /// Sp = Single Space
    ///
    /// status-line = HTTP-version SP status-code SP [ reason-phrase ]
    ///
    /// # Errors
    ///
    /// Returns Error if write fails
    pub async fn write_to<W: AsyncWriteExt + Unpin>(&self, mut w: W) -> Result<()> {
        let mut buf = Vec::new();

        write!(
            buf,
            "HTTP/{} {} {}\r\n",
            self.version,
            self.status_code.to_code(),
            self.status_code.to_reason()
        )?;

        w.write_all(&buf).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_status_line_write_to() -> Result<()> {
        let status_line = StatusLine::new(StatusCode::Ok);
        let mut buf = Vec::new();
        status_line.write_to(&mut buf).await?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\n".to_vec());

        Ok(())
    }
}
