use std::io::Read;

use crate::message::{
    error::{RequestError, RequestLineError},
    headers::Headers,
    request_line::RequestLine,
};

#[derive(Debug)]
pub struct Request {
    pub line: RequestLine,
    pub headers: Headers,
    body: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
enum ParserState {
    Done,
    RequestLine,
    Headers,
    Body,
    Error,
}

#[derive(Debug)]
pub struct RequestParser {
    request: Request,
    state: ParserState,
    body_len: Option<usize>,
}

impl RequestParser {
    fn find_body_len(&mut self) -> Result<(), RequestError> {
        if self.body_len.is_some() {
            // Already calculated
            return Ok(());
        }
        let Some(c) = self.request.headers.get("Content-Length") else {
            self.body_len = Some(0);
            return Ok(());
        };

        let cl = c.parse::<usize>();
        if let Ok(cl) = cl {
            self.body_len = Some(cl);
            return Ok(());
        }

        // if all values seperated by ',' is equal, and a number, then this value will be used
        let mut values = c.split(',').map(|v| v.trim());
        let Some(first) = values.next() else {
            return Err(RequestError::InvalidContentLength);
        };
        if !values.all(|v| v == first) {
            return Err(RequestError::InvalidContentLength);
        }
        let len = first
            .parse::<usize>()
            .map_err(|_| RequestError::InvalidContentLength)?;
        self.body_len = Some(len);
        Ok(())
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
                        // Is done if there are on content-length header
                        if self.request.headers.get("Content-Length").is_none() {
                            self.state = ParserState::Done;
                        } else {
                            self.state = ParserState::Body;
                        }
                    }
                }
                ParserState::Body => {
                    self.find_body_len()?;
                    let Some(len) = self.body_len else {
                        return Err(RequestError::InvalidContentLength);
                    };
                    println!("Body len: {}", len);
                    println!("Bytes len: {}, read: {}", bytes.len(), read);
                    let current_data = &bytes[read..];
                    println!("Current data: {}", String::from_utf8_lossy(&bytes[read..]));
                    self.request.body.extend_from_slice(current_data);

                    println!(
                        "Current body: {}",
                        String::from_utf8_lossy(&self.request.body)
                    );

                    if self.request.body.len() > len {
                        return Err(RequestError::BodyTooLong);
                    }

                    if self.request.body.len() == len {
                        self.state = ParserState::Done;
                    }

                    return Ok(bytes.len());
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
            body_len: None,
        };
        while parser.state != ParserState::Done {
            let n = reader.read(&mut buf[read..])?;
            // TODO: Handle EOF, ie. n = 0
            if n == 0 {
                eprint!("Read 0 bytes");
                println!("Buf: {}", String::from_utf8_lossy(&buf[..read]));
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
    fn test_content_length() -> Result<(), RequestError> {
        let mut parser = RequestParser {
            state: ParserState::RequestLine,
            request: Request {
                line: RequestLine::default(),
                headers: Headers::new(),
                body: Vec::new(),
            },
            body_len: None,
        };

        parser.request.headers.parse(b"Content-Length: 1\r\n")?;
        parser.find_body_len()?;
        assert_eq!(parser.body_len, Some(1));

        parser.body_len = None;
        parser.request.headers = Headers::new();
        parser.request.headers.parse(b"Content-Length: 2,2,2\r\n")?;
        parser.find_body_len()?;
        assert_eq!(parser.body_len, Some(2));

        parser.body_len = None;
        parser.request.headers = Headers::new();
        parser.request.headers.parse(b"Content-Length: 2,1,1\r\n")?;
        let res = parser.find_body_len();
        println!("Res: {:?}", res);
        assert!(res.is_err());

        Ok(())
    }
}
