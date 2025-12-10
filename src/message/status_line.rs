use std::io::{self, Write};

use tokio::io::AsyncWriteExt;

use crate::message::{error::StatusLineError, version::HttpVersion};

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

    pub fn parse(bytes: &[u8]) -> Result<StatusCode, StatusLineError> {
        match bytes {
            b"200" => Ok(Self::Ok),
            b"400" => Ok(Self::BadRequest),
            b"404" => Ok(Self::NotFound),
            b"405" => Ok(Self::MethodNotAllowed),
            b"500" => Ok(Self::InternalServerError),
            _ => Err(StatusLineError::InvalidStatusCode),
        }
    }
}

#[derive(Debug)]
pub struct StatusLine {
    pub version: HttpVersion,
    pub status_code: StatusCode,
}

impl StatusLine {
    pub fn new(status_code: StatusCode) -> StatusLine {
        StatusLine {
            version: HttpVersion::from((1, 1)),
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
    pub async fn write_to<W: AsyncWriteExt + Unpin>(&self, mut w: W) -> io::Result<()> {
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

    /// Follows RFC 9112 Section 4
    /// SP = Single Space
    ///
    /// status-line = HTTP-version SP status-code SP [ reason-phrase ]
    ///
    /// # Returns
    ///
    /// No CRLF => Ok(None)
    /// Valid data => Ok((StatusLine, data consumed))
    ///
    /// # Errors
    ///
    /// This function will return an error if it does not follow the above format
    pub fn from_line(line: &[u8]) -> Result<StatusLine, StatusLineError> {
        let parts = line.split(|&b| b == b' ').collect::<Vec<&[u8]>>();
        if parts.len() != 3 && parts.len() != 2 {
            return Err(StatusLineError::MalformedStatusLine);
        }

        let version_parts = parts[0].split(|&b| b == b'/').collect::<Vec<&[u8]>>();
        if version_parts.len() != 2 || version_parts[0] != b"HTTP" {
            return Err(StatusLineError::MalformedStatusLine);
        }
        let version = HttpVersion::from_bytes(version_parts[1])?;
        let status_code = StatusCode::parse(parts[1])?;

        Ok(StatusLine {
            version,
            status_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_status_line_write_to() -> io::Result<()> {
        let status_line = StatusLine::new(StatusCode::Ok);
        let mut buf = Vec::new();
        status_line.write_to(&mut buf).await?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\n".to_vec());

        Ok(())
    }

    #[test]
    fn test_status_line_parse() -> Result<(), StatusLineError> {
        let input = b"HTTP/1.1 200 Ok";
        let rl = StatusLine::from_line(input)?;
        assert_eq!(rl.version, (1, 1));
        assert_eq!(rl.status_code, StatusCode::Ok);

        let input = b"HTTP/1.1 200";
        let rl = StatusLine::from_line(input)?;
        assert_eq!(rl.version, (1, 1));
        assert_eq!(rl.status_code, StatusCode::Ok);

        let input = b"HTTP/1.1  200 Ok";
        let rl = StatusLine::from_line(input);

        assert!(rl.is_err());

        let input = b"HTP/1.1  200 Ok";
        let rl = StatusLine::from_line(input);

        assert!(rl.is_err());

        Ok(())
    }
}
