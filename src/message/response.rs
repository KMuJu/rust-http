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

#[derive(Debug, Eq, PartialEq)]
enum ParserState {
    Done,
    StatusLine,
    Headers,
    Body,
}

pub struct ResponseParser {
    response: Response,
    state: ParserState,
    body_parser: BodyParser,
}

impl ResponseParser {
    fn parse_body(&mut self, bytes: &[u8]) -> Result<usize, ResponseError> {
        let body = &mut self.response.body;
        let headers = &mut self.response.headers;
        let (size, done) = self.body_parser.parse_body(body, headers, bytes)?;
        if done {
            self.state = ParserState::Done;
        }

        Ok(size)
    }
    /// Takes in the data not yet consumed and gives it to the correct parsing function.
    /// Returns how much data was consumed.
    /// Ignores trailers. TODO: add support for them
    ///
    /// # Panics
    ///
    /// Panics if it has read data before parsing request line
    ///
    /// # Errors
    ///
    /// This function will return an error if a parsing function errors
    fn parse(&mut self, bytes: &[u8]) -> Result<usize, ResponseError> {
        let mut read = 0;
        // Loops until state is done
        loop {
            match self.state {
                ParserState::Done => break,
                ParserState::StatusLine => {
                    assert!(
                        read == 0,
                        "Request line is the first thing to parse, so read should be 0"
                    );
                    let rl = StatusLine::parse(bytes)?;
                    match rl {
                        None => return Ok(read),
                        Some((rl, size)) => {
                            self.state = ParserState::Headers;
                            self.response.status_line = rl;
                            read += size;
                        }
                    }
                }
                ParserState::Headers => {
                    let n = self.response.headers.parse_one(&bytes[read..])?;
                    if n == 0 {
                        return Ok(read);
                    }
                    read += n;

                    // Line is CRLF (\r\n)
                    if n == 2 {
                        self.state = ParserState::Body;
                        // Body state will check if it needs to parse anything
                    }
                }
                ParserState::Body => {
                    let n = self.parse_body(&bytes[read..])?;
                    if n == 0 {
                        return Ok(read);
                    }

                    read += n;
                }
            }
        }

        Ok(read)
    }

    /// Creates the response from the reader
    ///
    /// # Errors
    ///
    /// This function will return an error if receives EOF or if there is an error parsing the data
    pub async fn response_from_reader<R>(reader: &mut R) -> Result<Response, ResponseError>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = [0u8; 1024];
        let mut read: usize = 0;
        let mut parser = ResponseParser {
            state: ParserState::StatusLine,
            response: Response::new(StatusCode::Ok),
            body_parser: BodyParser::new(),
        };
        // This loop handle the reading, allowing the parse function to only worry about the data
        while parser.state != ParserState::Done {
            let n = reader.read(&mut buf[read..]).await?;
            // TODO: Handle EOF, ie. n = 0
            if n == 0 {
                eprint!("Read 0 bytes");
                return Err(ResponseError::MalformedResponse);
            }

            let consumed = parser.parse(&buf[..read + n])?;
            read += n;
            if consumed == 0 {
                continue;
            }

            // Moves the data not consumed to the front
            buf.copy_within(consumed.., 0);
            // Size of data not consumed is read - consumed
            read -= consumed;
        }
        Ok(parser.response)
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
        println!("Buf: {}", String::from_utf8_lossy(&buf).escape_debug());
        assert_eq!(
            buf,
            b"HTTP/1.1 200 Ok\r\ncontent-length: 5\r\ncontent-type: text/plain\r\n\r\nHello"
        );

        Ok(())
    }
}
