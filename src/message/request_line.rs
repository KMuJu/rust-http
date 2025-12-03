use std::io::{self, Write};
use tokio::io::AsyncWriteExt;

use crate::message::{Method, error::RequestLineError};

#[derive(Debug, PartialEq, Eq)]
pub struct RequestLine {
    pub method: Method,
    pub url: String,
    pub version: String,
}
const CRLF: &[u8; 2] = b"\r\n";

impl RequestLine {
    /// Follows RFC 9112 Section 3
    /// SP = Single Space
    ///
    /// request-line   = method SP request-target SP HTTP-version
    ///
    /// # Errors
    ///
    /// Returns Error if write fails
    pub async fn write_to<W: AsyncWriteExt + Unpin>(&self, mut w: W) -> io::Result<()> {
        let mut buf = Vec::new();

        write!(
            buf,
            "{} {} HTTP/{}\r\n",
            self.method.to_str(),
            self.url,
            self.version,
        )?;

        w.write_all(&buf).await?;
        Ok(())
    }

    /// Follows RFC 9112 Section 3
    /// SP = Single Space
    ///
    /// request-line   = method SP request-target SP HTTP-version
    ///
    /// # Returns
    ///
    /// No CRLF => Ok(None)
    /// Valid data => Ok((RequestLine, data consumed))
    ///
    /// # Errors
    ///
    /// This function will return an error if it does not follow the above format
    pub fn parse(bytes: &[u8]) -> Result<Option<(RequestLine, usize)>, RequestLineError> {
        let end_of_line = bytes.windows(CRLF.len()).position(|w| w == CRLF);
        let Some(end) = end_of_line else {
            return Ok(None);
        };
        let current_data = &bytes[..end];

        let parts = current_data.split(|&b| b == b' ').collect::<Vec<&[u8]>>();
        if parts.len() != 3 {
            return Err(RequestLineError::MalformedRequestLine);
        }

        let method = Method::parse(parts[0])?;
        let url = String::from_utf8_lossy(parts[1]).into_owned();
        let version_parts = parts[2].split(|&b| b == b'/').collect::<Vec<&[u8]>>();
        if version_parts.len() != 2 || version_parts[0] != b"HTTP" {
            return Err(RequestLineError::MalformedRequestLine);
        }

        let version = String::from_utf8_lossy(version_parts[1]).into_owned();

        Ok(Some((
            RequestLine {
                method,
                url,
                version,
            },
            end + CRLF.len(),
        )))
    }

    pub fn new(method: Method, url: String, version: String) -> RequestLine {
        RequestLine {
            method,
            url,
            version,
        }
    }
}
impl Default for RequestLine {
    fn default() -> RequestLine {
        RequestLine {
            method: Method::Get,
            url: "".to_string(),
            version: "1.1".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_request_line_parse() -> Result<(), RequestLineError> {
        let input = b"GET / HTTP/1.1";
        let output = RequestLine::parse(input)?;

        assert!(output.is_none());

        let input = b"GET / HTTP/1.1\r\n";
        let output = RequestLine::parse(input)?;
        let (rl, size) = output.unwrap();

        assert_eq!(rl.method, Method::Get);
        assert_eq!(rl.url, "/".to_string());
        assert_eq!(rl.version, "1.1".to_string());
        assert_eq!(size, 16);

        let input = b"POST /test HTTP/1.1\r\n";
        let output = RequestLine::parse(input)?;
        let (rl, size) = output.unwrap();

        assert_eq!(rl.method, Method::Post);
        assert_eq!(rl.url, "/test".to_string());
        assert_eq!(rl.version, "1.1".to_string());
        assert_eq!(size, 21);

        let input = b"POST  /test HTTP/1.1\r\n";
        let output = RequestLine::parse(input);

        assert!(output.is_err());

        let input = b"POST /test HTP/1.1\r\n";
        let output = RequestLine::parse(input);

        assert!(output.is_err());

        Ok(())
    }
}
