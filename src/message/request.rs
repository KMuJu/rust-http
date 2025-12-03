use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::message::{Headers, Method, RequestError, RequestLine, body::BodyParser};

#[derive(Debug)]
pub struct Request {
    pub line: RequestLine,
    pub headers: Headers,
    pub(crate) body: Vec<u8>,
}

impl Request {
    pub fn get_method(&self) -> &Method {
        &self.line.method
    }

    pub fn get_url(&self) -> &str {
        &self.line.url
    }

    pub fn get_body(&self) -> &[u8] {
        &self.body
    }

    /// Writes response into a writer.
    /// Is not a streamed request, so will update 'Content-Length' header to be correct
    ///
    /// # Errors
    ///
    /// Returns an error if any element fails to write
    pub async fn write_to<W: AsyncWriteExt + Unpin>(&mut self, mut w: W) -> io::Result<()> {
        self.line.write_to(&mut w).await?;
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
}

#[derive(Debug, Eq, PartialEq)]
enum ParserState {
    Done,
    RequestLine,
    Headers,
    Body,
}

#[derive(Debug)]
pub struct RequestParser {
    request: Request,
    state: ParserState,
    body_parser: BodyParser,
}

impl RequestParser {
    fn parse_body(&mut self, bytes: &[u8]) -> Result<usize, RequestError> {
        let body = &mut self.request.body;
        let headers = &mut self.request.headers;
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
    fn parse(&mut self, bytes: &[u8]) -> Result<usize, RequestError> {
        let mut read = 0;
        // Loops until state is done
        loop {
            match self.state {
                ParserState::Done => break,
                ParserState::RequestLine => {
                    assert!(
                        read == 0,
                        "Request line is the first thing to parse, so read should be 0"
                    );
                    let rl = RequestLine::parse(bytes)?;
                    match rl {
                        None => return Ok(read),
                        Some((rl, size)) => {
                            self.state = ParserState::Headers;
                            self.request.line = rl;
                            read += size;
                        }
                    }
                }
                ParserState::Headers => {
                    let n = self.request.headers.parse_one(&bytes[read..])?;
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

    /// Creates the request from the reader
    ///
    /// # Errors
    ///
    /// This function will return an error if receives EOF or if there is an error parsing the data
    pub async fn request_from_reader<R>(reader: &mut R) -> Result<Request, RequestError>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = [0u8; 1024];
        let mut read: usize = 0;
        let mut parser = RequestParser {
            state: ParserState::RequestLine,
            request: Request {
                line: RequestLine::default(),
                headers: Headers::new(),
                body: Vec::new(),
            },
            body_parser: BodyParser::new(),
        };
        // This loop handle the reading, allowing the parse function to only worry about the data
        while parser.state != ParserState::Done {
            let n = reader.read(&mut buf[read..]).await?;
            // TODO: Handle EOF, ie. n = 0
            if n == 0 {
                eprint!("Read 0 bytes");
                return Err(RequestError::MalformedRequest);
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
        Ok(parser.request)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    use super::*;
    use crate::message::method::Method;
    use crate::message::test_utils::batch_reader::BatchReader;

    #[tokio::test]
    async fn test_request_parser() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let rq = RequestParser::request_from_reader(&mut Cursor::new(input)).await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, "1.1".to_string());
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn test_request_parser_batch_no_body() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, "1.1".to_string());
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert!(rq.body.is_empty());

        let input = b"POST /post HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await?;
        assert_eq!(rq.line.method, Method::Post);
        assert_eq!(rq.line.url, "/post".to_string());
        assert_eq!(rq.line.version, "1.1".to_string());
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert!(rq.body.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_request_parser_batch_with_body() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 1\r\n\r\nA".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, "1.1".to_string());
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(rq.headers.get("Content-Length"), Some(&"1".to_string()));
        assert_eq!(rq.body, vec![b'A']);

        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 2\r\n\r\nA".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await;

        assert!(rq.is_err());
        match rq {
            Err(RequestError::MalformedRequest) => (),
            e => panic!("expected MalformedRequest, but was {:?}", e),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_chunked_encoding() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\nA\r\n1234567890\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1234567890");
        assert_eq!(rq.body.len(), 12);

        Ok(())
    }

    #[tokio::test]
    async fn test_chunked_encoding_with_crlf_in_body() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\n4\r\n1\r\n1\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1\r\n1");
        assert_eq!(rq.body.len(), 6);

        Ok(())
    }

    #[tokio::test]
    async fn test_chunked_encoding_err() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nABC\r\n4\r\n1234\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader).await;
        assert!(rq.is_err());

        Ok(())
    }
}
