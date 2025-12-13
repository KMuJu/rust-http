use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::message::{
    Headers, Request, RequestError, RequestLine, Response, ResponseError, StatusLine,
    body::parse_body, stream_reader::StreamReader,
};

pub struct Connection<R, W, T>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
    // T is the type that will be read from the reader
    // Will either be request or response
{
    reader: StreamReader<R>,
    writer: W,
    t: std::marker::PhantomData<T>,
}

impl<R, W, T> Connection<R, W, T>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: StreamReader::new(reader),
            writer,
            t: std::marker::PhantomData,
        }
    }
}

// Reads requests from the stream and sends responses
impl<R, W> Connection<R, W, Request>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    pub async fn read(&mut self) -> Result<Request, RequestError> {
        let req_line = {
            let line = self.reader.read_line().await?;
            RequestLine::from_line(&line)
        }?;

        let mut headers = Headers::new();
        loop {
            let line = self.reader.read_line().await?;
            if line.is_empty() {
                break;
            }

            headers.parse_one_from_line(&line)?;
        }

        let body = parse_body(&mut headers, &mut self.reader).await?;

        Ok(Request {
            line: req_line,
            headers,
            body,
        })
    }

    pub async fn respond(&mut self, response: &mut Response) -> io::Result<()> {
        response.write_to(&mut self.writer).await?;
        self.writer.flush().await
    }
}

// Reads reponses from the stream and sends requests
impl<R, W> Connection<R, W, Response>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    pub async fn read(&mut self) -> Result<Response, ResponseError> {
        let status_line = {
            let line = self.reader.read_line().await?;
            StatusLine::from_line(&line)
        }?;

        let mut headers = Headers::new();
        loop {
            let line = self.reader.read_line().await?;
            if line.is_empty() {
                break;
            }

            headers.parse_one_from_line(&line)?;
        }

        let body = parse_body(&mut headers, &mut self.reader).await?;

        Ok(Response {
            status_line,
            headers,
            body,
        })
    }

    pub async fn send(&mut self, request: &mut Request) -> io::Result<()> {
        request.write_to(&mut self.writer).await?;
        self.writer.flush().await
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::message::{
        Method, StatusCode, error::BodyError, test_utils::batch_reader::BatchReader,
    };

    use super::*;

    #[tokio::test]
    async fn test_request_connection() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let c = Cursor::new(input);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(c, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert_eq!(rq.body.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_request_connection_no_body() -> Result<(), RequestError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert!(rq.body.is_empty());

        let input = b"POST /post HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.line.method, Method::Post);
        assert_eq!(rq.line.url, "/post".to_string());
        assert_eq!(rq.line.version, (1, 1));
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
    async fn test_request_connection_batch_with_body() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 1\r\n\r\nA".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.line.method, Method::Get);
        assert_eq!(rq.line.url, "/".to_string());
        assert_eq!(rq.line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(rq.headers.get("Content-Length"), Some(&"1".to_string()));
        assert_eq!(rq.body, vec![b'A']);

        let input =
            b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nContent-Length: 2\r\n\r\nA".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await;

        assert!(rq.is_err());
        match rq {
            Err(RequestError::Body(BodyError::IO(_))) => (),
            e => panic!("expected Body IO Error, but was {:?}", e),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_request_connection_chunked_encoding() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\nA\r\n1234567890\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1234567890");
        assert_eq!(rq.body.len(), 12);

        Ok(())
    }

    #[tokio::test]
    async fn test_request_connection_chunked_encoding_with_crlf_in_body() -> Result<(), RequestError>
    {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\n4\r\n1\r\n1\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1\r\n1");
        assert_eq!(rq.body.len(), 6);

        Ok(())
    }

    #[tokio::test]
    async fn test_request_connection_chunked_encoding_err() -> Result<(), RequestError> {
        let input =
            b"GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nABC\r\n4\r\n1234\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Request>::new(batch_reader, writer);

        let rq = connection.read().await;
        assert!(rq.is_err());

        Ok(())
    }

    //
    //  Response tests
    //

    #[tokio::test]
    async fn test_response_connection() -> Result<(), ResponseError> {
        let input = b"HTTP/1.1 200 Ok\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n";
        let c = Cursor::new(input);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(c, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.status_line.status_code, StatusCode::Ok);
        assert_eq!(rq.status_line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert_eq!(rq.body.len(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_response_connection_no_body() -> Result<(), ResponseError> {
        let input = b"HTTP/1.1 200 Ok\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.status_line.status_code, StatusCode::Ok);
        assert_eq!(rq.status_line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(
            rq.headers.get("User-Agent"),
            Some(&"curl/7.81.0".to_string())
        );
        assert_eq!(rq.headers.get("Accept"), Some(&"*/*".to_string()));
        assert!(rq.body.is_empty());

        let input = b"HTTP/1.1 404 Not Found\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.status_line.status_code, StatusCode::NotFound);
        assert_eq!(rq.status_line.version, (1, 1));
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
    async fn test_response_connection_batch_with_body() -> Result<(), ResponseError> {
        let input =
            b"HTTP/1.1 200 Ok\r\nHost: localhost:42069\r\nContent-Length: 1\r\n\r\nA".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(rq.status_line.status_code, StatusCode::Ok);
        assert_eq!(rq.status_line.version, (1, 1));
        assert_eq!(rq.headers.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(rq.headers.get("Content-Length"), Some(&"1".to_string()));
        assert_eq!(rq.body, vec![b'A']);

        let input =
            b"HTTP/1.1 200 Ok\r\nHost: localhost:42069\r\nContent-Length: 2\r\n\r\nA".to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await;

        assert!(rq.is_err());
        match rq {
            Err(ResponseError::Body(BodyError::IO(_))) => (),
            e => panic!("expected Body IO Error, but was {:?}", e),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_response_connection_chunked_encoding() -> Result<(), ResponseError> {
        let input =
            b"HTTP/1.1 200 Ok\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\nA\r\n1234567890\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1234567890");
        assert_eq!(rq.body.len(), 12);

        Ok(())
    }

    #[tokio::test]
    async fn test_response_connection_chunked_encoding_with_crlf_in_body()
    -> Result<(), ResponseError> {
        let input =
            b"HTTP/1.1 200 Ok\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nAB\r\n4\r\n1\r\n1\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await?;
        assert_eq!(String::from_utf8_lossy(&rq.body), "AB1\r\n1");
        assert_eq!(rq.body.len(), 6);

        Ok(())
    }

    #[tokio::test]
    async fn test_response_connection_chunked_encoding_err() -> Result<(), ResponseError> {
        let input =
            b"HTTP/1.1 200 Ok\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nABC\r\n4\r\n1234\r\n0\r\n\r\n"
                .to_vec();
        let batch_reader = BatchReader::new(input.clone(), 3);
        let writer = Cursor::new(input.to_vec());
        let mut connection = Connection::<_, _, Response>::new(batch_reader, writer);

        let rq = connection.read().await;
        assert!(rq.is_err());

        Ok(())
    }
}
