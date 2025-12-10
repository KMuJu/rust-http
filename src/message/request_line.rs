use std::io::{self, Write};
use tokio::io::AsyncWriteExt;

use crate::message::{Method, error::RequestLineError, version::HttpVersion};

#[derive(Debug, PartialEq, Eq)]
pub struct RequestLine {
    pub method: Method,
    pub url: String,
    pub version: HttpVersion,
}

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
    pub fn from_line(line: &[u8]) -> Result<RequestLine, RequestLineError> {
        let parts = line.split(|&b| b == b' ').collect::<Vec<&[u8]>>();
        if parts.len() != 3 {
            return Err(RequestLineError::MalformedRequestLine);
        }

        let method = Method::parse(parts[0])?;
        let url = String::from_utf8_lossy(parts[1]).into_owned();
        let version_parts = parts[2].split(|&b| b == b'/').collect::<Vec<&[u8]>>();
        if version_parts.len() != 2 || version_parts[0] != b"HTTP" {
            return Err(RequestLineError::MalformedRequestLine);
        }

        let version = HttpVersion::from_bytes(version_parts[1])?;

        Ok(RequestLine {
            method,
            url,
            version,
        })
    }

    pub fn from_parts(method: Method, url: String, version: HttpVersion) -> RequestLine {
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
            version: HttpVersion::default(),
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
        let rl = RequestLine::from_line(input)?;

        assert_eq!(rl.method, Method::Get);
        assert_eq!(rl.url, "/".to_string());
        assert_eq!(rl.version, (1, 1));

        let input = b"POST /test HTTP/1.1";
        let rl = RequestLine::from_line(input)?;

        assert_eq!(rl.method, Method::Post);
        assert_eq!(rl.url, "/test".to_string());
        assert_eq!(rl.version, (1, 1));

        let input = b"POST  /test HTTP/1.1";
        let rl = RequestLine::from_line(input);

        assert!(rl.is_err());

        let input = b"POST /test HTP/1.1";
        let rl = RequestLine::from_line(input);

        assert!(rl.is_err());

        Ok(())
    }
}
