use crate::message::{
    Headers,
    error::{BodyError, HeadersError},
};

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
pub struct BodyParser {
    encoding: Option<Encoding>,
    chunked_state: ChunkedState,
    size_parsed: usize,
}

const CRLF: &[u8; 2] = b"\r\n";

impl BodyParser {
    pub(crate) fn new() -> BodyParser {
        BodyParser {
            encoding: None,
            chunked_state: ChunkedState::Size,
            size_parsed: 0,
        }
    }

    /// Sets the encoding type of the parser
    ///
    /// Follows https://datatracker.ietf.org/doc/html/rfc9112#name-message-body-length
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn set_encoding(&mut self, headers: &mut Headers) -> Result<(), BodyError> {
        if self.encoding.is_some() {
            return Ok(());
        }
        let transmission = headers.get("Transfer-Encoding");
        let content = headers.get("Content-Length");

        if transmission.is_some() && content.is_some() {
            return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
        }

        if let Some(transmission) = transmission {
            if transmission == "chunked" {
                self.encoding = Some(Encoding::Chunked);
            } else {
                return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
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
                return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
            };
            if !values.all(|v| v == first) {
                return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
            }
            let len = first
                .parse::<usize>()
                .map_err(|_| BodyError::Header(HeadersError::InvalidContentLength))?;
            self.encoding = Some(Encoding::Nothing(len));
        } else {
            self.encoding = Some(Encoding::Nothing(0))
        }

        Ok(())
    }

    /// Parses the chunked body, following 7.1 in RFC9112
    /// Example pattern (with different stuff on different lines)
    ///
    /// 2\r\n
    /// AB\r\n
    /// A\r\n
    /// 1234567890\r\n
    /// 0\r\n
    ///
    /// # Errors
    ///
    /// This function will return an error if there is an invalid chunk-size
    fn parse_chunked_body(
        &mut self,
        body: &mut Vec<u8>,
        headers: &mut Headers,
        bytes: &[u8],
    ) -> Result<(usize, bool), BodyError> {
        let crlf = bytes.windows(CRLF.len()).position(|w| w == CRLF);
        let end_of_line = match crlf {
            Some(e) => e,
            None => return Ok((0, false)),
        };

        let mut done = false;
        match self.chunked_state {
            ChunkedState::Size => {
                // TODO: Implement chunk extensions
                // Currently ignores them
                //
                // Assumes that chunked size is never longer than buffersize
                let size_line = &bytes[..end_of_line];
                match usize::from_str_radix(&String::from_utf8_lossy(size_line), 16) {
                    Ok(size) => {
                        self.chunked_state = ChunkedState::Data(size);
                        self.size_parsed = 0;

                        // Then we are done
                        if size == 0 {
                            done = true;

                            let len = { body.len() };
                            headers.set("Content-Length", len.to_string());

                            // TODO: Will need to change if server supports more encodings
                            // Is supposed to removed chunked from the header, but for now only
                            // chunked is supported
                            headers.remove("Transfer-Encoding");
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing chunked-size: {e}");
                        return Err(BodyError::MalformedChunkedSize);
                    }
                }
            }
            ChunkedState::Data(size) => {
                // checks if the bytes after size is crlf
                let found_end = bytes.len() > size - self.size_parsed + 1
                    && (bytes[size - self.size_parsed] == b'\r'
                        && bytes[size + 1 - self.size_parsed] == b'\n'); // found end of data

                if !found_end && bytes.len() > size - self.size_parsed + 1 {
                    println!("Bytes after size is not crlf");
                    return Err(BodyError::MalformedChunkedBody);
                }

                let mut end = if found_end {
                    size - self.size_parsed
                } else {
                    bytes.len()
                };

                let b = &bytes[..end];
                body.extend_from_slice(b);
                self.size_parsed += end;

                if found_end {
                    self.chunked_state = ChunkedState::Size;
                    end += CRLF.len();
                }

                return Ok((end, false));
            }
        }

        Ok((end_of_line + CRLF.len(), done))
    }

    /// Finds encoding type, then parses the incomming bytes based on that
    ///
    /// # Errors
    ///
    /// This function will return an error if it receives data that is longer than Content-Length,
    /// or if there is invalid combination of headers
    pub fn parse_body(
        &mut self,
        body: &mut Vec<u8>,
        headers: &mut Headers,
        bytes: &[u8],
    ) -> Result<(usize, bool), BodyError> {
        self.set_encoding(headers)?;
        match self.encoding {
            // No body
            Some(Encoding::Nothing(0)) => {
                // self.state = ParserState::Done;
                Ok((0, true))
            }
            Some(Encoding::Nothing(len)) => {
                // TODO: Should this error if the read includes bytes not part of body?
                let remaining = len.saturating_sub(body.len());
                if remaining == 0 {
                    return Ok((0, true)); // already complete
                }

                // consume at most `remaining` bytes
                let to_take = remaining.min(bytes.len());
                body.extend_from_slice(&bytes[..to_take]);

                let done = body.len() == len;
                Ok((to_take, done))
            }
            Some(Encoding::Chunked) => self.parse_chunked_body(body, headers, bytes),
            None => Err(BodyError::Header(HeadersError::InvalidContentLength)), // TODO: Find better error type?
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::message::RequestError;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_set_content_length() -> Result<(), RequestError> {
        let mut headers = Headers::new();
        let mut parser = BodyParser::new();

        headers.parse_one(b"Content-Length: 1\r\n")?;
        parser.set_encoding(&mut headers)?;
        assert_eq!(parser.encoding, Some(Encoding::Nothing(1)));

        parser.encoding = None;
        headers = Headers::new();
        headers.parse_one(b"Content-Length: 2,2,2\r\n")?;
        parser.set_encoding(&mut headers)?;
        assert_eq!(parser.encoding, Some(Encoding::Nothing(2)));

        parser.encoding = None;
        headers = Headers::new();
        headers.parse_one(b"Content-Length: 2,1,1\r\n")?;
        let res = parser.set_encoding(&mut headers);
        assert!(res.is_err());
        assert_eq!(parser.encoding, None);

        parser.encoding = None;
        headers = Headers::new();
        headers.parse_one(b"Transfer-Encoding: chunked\r\n")?;
        parser.set_encoding(&mut headers)?;
        assert_eq!(parser.encoding, Some(Encoding::Chunked));

        parser.encoding = None;
        headers = Headers::new();
        headers.parse_one(b"Content-Length: 2\r\n")?;
        headers.parse_one(b"Transfer-Encoding: chunked\r\n")?;
        let res = parser.set_encoding(&mut headers);
        assert!(res.is_err());
        assert_eq!(parser.encoding, None);

        Ok(())
    }

    #[test]
    fn test_parse_body_content_length() -> Result<(), RequestError> {
        let mut headers = Headers::new();
        let mut body = Vec::new();
        let mut parser = BodyParser::new();

        headers.parse_one(b"Content-Length: 7\r\n")?;
        let input = b"testing".to_vec();
        parser.parse_body(&mut body, &mut headers, &input)?;
        assert_eq!(body, b"testing".to_vec());

        // TODO: Should it error?? Could be a new request
        // headers = Headers::new();
        // headers.parse_one(b"Content-Length: 3\r\n")?;
        // let input = b"testing".to_vec();
        // let res = parser.parse_body(&mut body, &mut headers, &input);
        // assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn test_parse_body_chunked() -> Result<(), RequestError> {
        let mut headers = Headers::new();
        let mut body = Vec::new();
        let mut parser = BodyParser::new();
        headers.parse_one(b"Transfer-Encoding: chunked\r\n")?;
        let input = b"1\r\n".to_vec();
        parser.parse_body(&mut body, &mut headers, &input)?;
        let input = b"A\r\n".to_vec();
        parser.parse_body(&mut body, &mut headers, &input)?;
        let input = b"3\r\n".to_vec();
        parser.parse_body(&mut body, &mut headers, &input)?;
        let input = b"BCD\r\n".to_vec();
        parser.parse_body(&mut body, &mut headers, &input)?;
        assert_eq!(
            String::from_utf8_lossy(&body),
            String::from_utf8_lossy(b"ABCD")
        );

        Ok(())
    }
}
