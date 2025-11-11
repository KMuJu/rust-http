use std::{
    fs,
    io::{self, Result, Write},
};

use crate::message::{Headers, StatusCode, StatusLine};

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

    /// Writes response into a writer.
    /// Will update 'Content-Length' header to be correct
    ///
    /// # Errors
    ///
    /// Returns an error if any element fails to write
    pub fn write_to<W: Write>(&mut self, mut w: W) -> Result<()> {
        self.status_line.write_to(&mut w)?;
        if !self.body.is_empty() {
            self.headers
                .add("Content-Length", self.body.len().to_string());
        }
        self.headers.write_to(&mut w)?;
        if !self.body.is_empty() {
            w.write_all(&self.body)?;
        }

        Ok(())
    }

    pub fn internal_error() -> Response {
        Response {
            status_line: StatusLine::new(StatusCode::InternalServerError),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

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
impl Write for Response {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.body.write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        self.body.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_write_response() -> Result<()> {
        let mut response = Response::new(StatusCode::Ok);
        response.headers = Headers::new(); // Remove default headers, these can change
        let mut buf = Vec::new();
        response.write_to(&mut buf)?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\n");

        response.headers.add("Content-Type", "text/plain");
        buf = Vec::new();
        response.write_to(&mut buf)?;
        assert_eq!(buf, b"HTTP/1.1 200 Ok\r\ncontent-type: text/plain\r\n\r\n");

        buf = Vec::new();
        response.body.write_all(b"Hello")?;
        response.write_to(&mut buf)?;
        println!("Buf: {}", String::from_utf8_lossy(&buf).escape_debug());
        assert_eq!(
            buf,
            b"HTTP/1.1 200 Ok\r\ncontent-length: 5\r\ncontent-type: text/plain\r\n\r\nHello"
        );

        Ok(())
    }
}
