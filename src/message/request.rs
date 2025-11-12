use std::{io::Read, usize};

use crate::message::{Headers, Method, RequestError, RequestLine};

#[derive(Debug)]
pub struct Request {
    pub line: RequestLine,
    pub headers: Headers,
    body: Vec<u8>,
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
}

#[derive(Debug, Eq, PartialEq)]
enum ParserState {
    Done,
    RequestLine,
    Headers,
    Body,
    Error,
}

/// Different encoding types supported
#[derive(Debug, PartialEq, Eq)]
enum Encoding {
    Nothing(usize), // Stores the size of the body. No body is size 0
    Chunked,
}

/// Used to store state for parsing chunked body
#[derive(Debug, PartialEq, Eq)]
enum ChunkedState {
    Size,        // Going to parse the size
    Data(usize), // Going to parse the body
}

#[derive(Debug)]
pub struct RequestParser {
    request: Request,
    state: ParserState,
    encoding: Option<Encoding>,
    chuncked_state: ChunkedState,
}

const CRLF: &[u8; 2] = b"\r\n";

impl RequestParser {
    fn set_encoding(&mut self) -> Result<(), RequestError> {
        if self.encoding.is_some() {
            return Ok(());
        }
        let transmission = self.request.headers.get("Transfer-Encoding");
        let content = self.request.headers.get("Content-Length");

        if transmission.is_some() && content.is_some() {
            return Err(RequestError::InvalidHeaderFields);
        }

        if let Some(transmission) = transmission {
            if transmission == "chunked" {
                self.encoding = Some(Encoding::Chunked);
            } else {
                return Err(RequestError::InvalidHeaderFields);
            }
        } else if let Some(length) = content {
            let cl = length.parse::<usize>();
            if let Ok(cl) = cl {
                self.encoding = Some(Encoding::Nothing(cl));
                return Ok(());
            }

            // if all values seperated by ',' is equal, and a number, then this value will be used
            let mut values = length.split(',').map(|v| v.trim());
            let Some(first) = values.next() else {
                return Err(RequestError::InvalidContentLength);
            };
            if !values.all(|v| v == first) {
                return Err(RequestError::InvalidContentLength);
            }
            let len = first
                .parse::<usize>()
                .map_err(|_| RequestError::InvalidContentLength)?;
            self.encoding = Some(Encoding::Nothing(len));
        } else {
            self.encoding = Some(Encoding::Nothing(0))
        }

        Ok(())
    }

    fn parse_chunked_body(&mut self, bytes: &[u8]) -> Result<usize, RequestError> {
        let end_of_line = bytes.windows(CRLF.len()).position(|w| w == CRLF);
        let Some(end_of_line) = end_of_line else {
            return Ok(0);
        };
        match self.chuncked_state {
            ChunkedState::Size => {
                // TODO: Implement chunk extensions
                // Currently ignores them
                let size_line = &bytes[..end_of_line];
                match usize::from_str_radix(&String::from_utf8_lossy(size_line), 16) {
                    Ok(size) => {
                        self.chuncked_state = ChunkedState::Data(size);

                        if size == 0 {
                            self.state = ParserState::Done;
                            self.request
                                .headers
                                .set("Content-Length", self.request.body.len().to_string());

                            // TODO: Will need to change if server supports more encodings
                            self.request.headers.remove("Transfer-Encoding");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing chunked-size: {e}");
                        return Err(RequestError::MalformedBody);
                    }
                }
            }
            ChunkedState::Data(size) => {
                if size + 2 >= bytes.len() {
                    return Ok(0);
                }

                if bytes[size] != b'\r' && bytes[size + 1] != b'\n' {
                    return Err(RequestError::MalformedBody);
                }

                let body = &bytes[..size];
                self.request.body.extend_from_slice(body);
                self.chuncked_state = ChunkedState::Size;
                return Ok(size + CRLF.len());
            }
        }

        Ok(end_of_line + CRLF.len())
    }

    fn parse_body(&mut self, bytes: &[u8]) -> Result<usize, RequestError> {
        self.set_encoding()?;
        match self.encoding {
            // No body
            Some(Encoding::Nothing(0)) => {
                self.state = ParserState::Done;
                Ok(0)
            }
            Some(Encoding::Nothing(len)) => {
                if self.request.body.len() + bytes.len() > len {
                    return Err(RequestError::BodyTooLong);
                }

                self.request.body.extend_from_slice(bytes);

                if self.request.body.len() == len {
                    self.state = ParserState::Done;
                }

                Ok(bytes.len())
            }
            Some(Encoding::Chunked) => self.parse_chunked_body(bytes),
            None => Err(RequestError::InvalidHeaderFields), // TODO: Find better error type?
        }
    }

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
                    let n = self.request.headers.parse(&bytes[read..])?;
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
                ParserState::Error => {}
            }
        }

        Ok(read)
    }

    pub fn request_from_reader(reader: &mut impl Read) -> Result<Request, RequestError> {
        let mut buf = [0u8; 1024];
        let mut read: usize = 0;
        let mut parser = RequestParser {
            state: ParserState::RequestLine,
            request: Request {
                line: RequestLine::default(),
                headers: Headers::new(),
                body: Vec::new(),
            },
            encoding: None,
            chuncked_state: ChunkedState::Size,
        };
        while parser.state != ParserState::Done {
            let n = reader.read(&mut buf[read..])?;
            // TODO: Handle EOF, ie. n = 0
            if n == 0 {
                eprint!("Read 0 bytes");
                println!(
                    "Buf: {}",
                    String::from_utf8_lossy(&buf[..read]).escape_debug()
                );
                return Err(RequestError::MalformedRequest);
            }
            let size = parser.parse(&buf[..read + n])?;
            read += n;
            if size == 0 {
                continue;
            }
            buf.copy_within(size.., 0);
            read -= size;
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

    #[test]
    fn test_request_parser() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let rq = RequestParser::request_from_reader(&mut Cursor::new(input))?;
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

    #[test]
    fn test_request_parser_batch_no_body() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader)?;
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
        let rq = RequestParser::request_from_reader(&mut batch_reader)?;
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

    #[test]
    fn test_request_parser_batch_with_body() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 1\r\n\r\nA".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader)?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, "1.1".to_string());
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(rq.headers.get("Content-Length"), Some(&"1".to_string()));
        assert_eq!(rq.body, vec![b'A']);

        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 2\r\n\r\nA".to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader);

        assert!(rq.is_err());
        match rq {
            Err(RequestError::MalformedRequest) => (),
            e => panic!("expected MalformedRequest, but was {:?}", e),
        }

        Ok(())
    }

    #[test]
    fn test_set_content_length() -> Result<(), RequestError> {
        let mut parser = RequestParser {
            state: ParserState::RequestLine,
            request: Request {
                line: RequestLine::default(),
                headers: Headers::new(),
                body: Vec::new(),
            },
            encoding: None,
            chuncked_state: ChunkedState::Size,
        };

        parser.request.headers.parse(b"Content-Length: 1\r\n")?;
        parser.set_encoding()?;
        assert_eq!(parser.encoding, Some(Encoding::Nothing(1)));

        parser.encoding = None;
        parser.request.headers = Headers::new();
        parser.request.headers.parse(b"Content-Length: 2,2,2\r\n")?;
        parser.set_encoding()?;
        assert_eq!(parser.encoding, Some(Encoding::Nothing(2)));

        parser.encoding = None;
        parser.request.headers = Headers::new();
        parser.request.headers.parse(b"Content-Length: 2,1,1\r\n")?;
        let res = parser.set_encoding();
        assert!(res.is_err());
        assert_eq!(parser.encoding, None);

        parser.encoding = None;
        parser.request.headers = Headers::new();
        parser
            .request
            .headers
            .parse(b"Transfer-Encoding: chunked\r\n")?;
        parser.set_encoding()?;
        assert_eq!(parser.encoding, Some(Encoding::Chunked));

        parser.encoding = None;
        parser.request.headers = Headers::new();
        parser.request.headers.parse(b"Content-Length: 2\r\n")?;
        parser
            .request
            .headers
            .parse(b"Transfer-Encoding: chunked\r\n")?;
        let res = parser.set_encoding();
        assert!(res.is_err());
        assert_eq!(parser.encoding, None);

        Ok(())
    }

    #[test]
    fn test_chunked_encoding() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\nA\r\n1234567890\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader)?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1234567890");
        assert_eq!(rq.body.len(), 12);

        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\n4\r\n1\r\n1\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader)?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1\r\n1");
        assert_eq!(rq.body.len(), 6);

        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nABC\r\n4\r\n1234\r\n0\r\n\r\n"
                .to_vec();
        let mut batch_reader = BatchReader::new(input, 3);
        let rq = RequestParser::request_from_reader(&mut batch_reader);
        assert!(rq.is_err());

        Ok(())
    }
}
