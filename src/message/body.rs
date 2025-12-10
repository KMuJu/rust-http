use tokio::io::AsyncReadExt;

use crate::message::{
    Headers,
    error::{BodyError, HeadersError},
    stream_reader::StreamReader,
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

const CRLF: &[u8; 2] = b"\r\n";

/// Returns the encoding type of the parser
///
/// Follows https://datatracker.ietf.org/doc/html/rfc9112#name-message-body-length
///
/// # Errors
///
/// This function will return an error if .
fn get_encoding(headers: &mut Headers) -> Result<Encoding, BodyError> {
    let transmission = headers.get("Transfer-Encoding");
    let content = headers.get("Content-Length");

    if transmission.is_some() && content.is_some() {
        return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
    }

    if let Some(transmission) = transmission {
        if transmission == "chunked" {
            return Ok(Encoding::Chunked);
        } else {
            return Err(BodyError::Header(HeadersError::InvalidHeaderFields));
        }
    } else if let Some(length) = content {
        let cl = length.parse::<usize>();
        if let Ok(cl) = cl {
            return Ok(Encoding::Nothing(cl));
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
        return Ok(Encoding::Nothing(len));
    }

    Ok(Encoding::Nothing(0))
}

pub async fn parse_body<R>(
    headers: &mut Headers,
    reader: &mut StreamReader<R>,
) -> Result<Vec<u8>, BodyError>
where
    R: AsyncReadExt + Unpin,
{
    let encoding = get_encoding(headers)?;
    match encoding {
        // No body
        Encoding::Nothing(0) => Ok(Vec::new()),
        Encoding::Nothing(len) => {
            // Simply read len bytes from the stream
            Ok(reader.read_n(len).await?)
        }
        Encoding::Chunked => {
            let mut state = ChunkedState::Size;
            let mut body = Vec::new();
            loop {
                match state {
                    ChunkedState::Size => {
                        let line = reader.read_line().await?;
                        match usize::from_str_radix(&String::from_utf8_lossy(&line), 16) {
                            Ok(size) => {
                                state = ChunkedState::Data(size);
                                if size == 0 {
                                    let len = { body.len() };
                                    headers.set("Content-Length", len.to_string());

                                    // TODO: Will need to change if server supports more encodings
                                    // Is supposed to removed chunked from the header, but for now only
                                    // chunked is supported
                                    headers.remove("Transfer-Encoding");
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Error parsing chunked-size: {e}");
                                return Err(BodyError::MalformedChunkedSize);
                            }
                        }
                    }
                    ChunkedState::Data(len) => {
                        let chunk = reader.read_n(len + CRLF.len()).await?;
                        if chunk[len] != b'\r' && chunk[len + 1] != b'\n' {
                            return Err(BodyError::MalformedChunkedBody);
                        }
                        body.extend_from_slice(&chunk[..len]);

                        state = ChunkedState::Size;
                    }
                }
            }

            Ok(body)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::message::RequestError;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_set_content_length() -> Result<(), RequestError> {
        let mut headers = Headers::new();

        headers.parse_one_from_line(b"Content-Length: 1")?;
        let encoding = get_encoding(&mut headers)?;
        assert_eq!(encoding, Encoding::Nothing(1));

        headers = Headers::new();
        headers.parse_one_from_line(b"Content-Length: 2,2,2")?;
        let encoding = get_encoding(&mut headers)?;
        assert_eq!(encoding, Encoding::Nothing(2));

        headers = Headers::new();
        headers.parse_one_from_line(b"Content-Length: 2,1,1")?;
        let res = get_encoding(&mut headers);
        assert!(res.is_err());

        headers = Headers::new();
        headers.parse_one_from_line(b"Transfer-Encoding: chunked")?;
        let encoding = get_encoding(&mut headers)?;
        assert_eq!(encoding, Encoding::Chunked);

        headers = Headers::new();
        headers.parse_one_from_line(b"Content-Length: 2")?;
        headers.parse_one_from_line(b"Transfer-Encoding: chunked")?;
        let res = get_encoding(&mut headers);
        assert!(res.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_body_chunked_() -> Result<(), RequestError> {
        let mut c = Cursor::new(b"1\r\nA\r\n4\r\n1\r\n1\r\n0\r\n");
        let mut reader = StreamReader::new(&mut c);
        let mut headers = Headers::new();
        headers.parse_one_from_line(b"Transfer-Encoding: chunked")?;
        let body = parse_body(&mut headers, &mut reader).await?;

        assert_eq!(String::from_utf8_lossy(&body), "A1\r\n1".to_string());

        Ok(())
    }
}
