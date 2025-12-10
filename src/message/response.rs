use std::{
    fs,
    io::{self},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::message::{Headers, ResponseError, StatusCode, StatusLine, body::BodyParser};

#[derive(Debug)]
pub struct Response {
    pub status_line: StatusLine,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: StatusCode) -> Response {
        Response {
            status_line: StatusLine::new(status_code),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    /// Writes request into a writer.
    /// Is not a streamed response, so will update 'Content-Length' header to be correct
    ///
    /// # Errors
    ///
    /// Returns an error if any element fails to write
    pub async fn write_to<W: AsyncWriteExt + Unpin>(&mut self, mut w: W) -> io::Result<()> {
        self.status_line.write_to(&mut w).await?;
        if !self.body.is_empty() {
            self.headers
                .set("Content-Length", self.body.len().to_string());
        }
        self.headers.write_to(&mut w).await?;
        if !self.body.is_empty() {
            w.write_all(&self.body).await?;
        }

        Ok(())
    }

    pub fn internal_error() -> Response {
        Response {
            status_line: StatusLine::new(StatusCode::InternalServerError),
            headers: Headers::new(), // TODO: Add headers??
            body: Vec::new(),
        }
    }

    /// Creates response from file
    ///
    /// # Errors
    ///
    /// This function will return an error if it fails to read from the file
    pub fn from_file(filename: &str, content_type: &str) -> io::Result<Response> {
        let filecontent = fs::read(filename)?;
        let mut headers = Headers::new();
        headers.add("Content-Length", filecontent.len().to_string());
        headers.add("Content-Type", content_type);
        Ok(Response {
            status_line: StatusLine::new(StatusCode::Ok),
            headers,
            body: filecontent,
        })
    }
}

// TODO: Is this stupid??
// Might also just provide body as the writer in the handlers
impl io::Write for Response {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut self.body, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_write_response() -> io::Result<()> {
        let mut response = Response::new(StatusCode::Ok);
        response.headers = Headers::new(); // Remove default headers, these can change
        let mut buf = Vec::new();
        response.write_to(&mut buf).await?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\n\r\n");

        response.headers.add("Content-Type", "text/plain");
        buf = Vec::new();
        response.write_to(&mut buf).await?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\ncontent-type: text/plain\r\n\r\n");

        buf = Vec::new();
        response.body.write_all(b"Hello").await?;
        response.write_to(&mut buf).await?;
        assert_eq!(
            buf,
            b"HTTP/1.1 200 Ok\r\ncontent-length: 5\r\ncontent-type: text/plain\r\n\r\nHello"
        );

        Ok(())
    }
}
