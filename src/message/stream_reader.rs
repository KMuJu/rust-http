use pin_project_lite::pin_project;
use tokio::io::{self, AsyncRead, AsyncReadExt};

use crate::message::error::StreamError;

pin_project! {
    pub struct StreamReader<R> {
        read: usize,
        buf: [u8; 2048],
        #[pin]
        reader: R,
    }
}

const CRLF: &[u8; 2] = b"\r\n";

impl<R: AsyncReadExt + Unpin> StreamReader<R> {
    pub fn new(reader: R) -> Self {
        StreamReader {
            read: 0,
            buf: [0u8; 2048],
            reader,
        }
    }

    pub async fn read_line(&mut self) -> Result<Vec<u8>, StreamError> {
        let mut out = Vec::new();
        let mut last_was_carrage_return = false;
        loop {
            for (i, &b) in self.buf[..self.read].iter().enumerate() {
                if last_was_carrage_return && b == b'\n' {
                    out.pop();
                    let index = (i + 1).min(self.buf.len());
                    self.buf.copy_within(index.., 0);
                    self.read -= index;
                    return Ok(out);
                }

                out.push(b);

                last_was_carrage_return = b == b'\r';
            }

            let n = self.reader.read(&mut self.buf).await?;
            if n == 0 {
                return Err(StreamError::EOF);
            }
            self.read = n;
        }
    }
}

impl<R: AsyncReadExt + Unpin> AsyncRead for StreamReader<R> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let this = self.project();
        this.reader.poll_read(cx, buf)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn test_stream_reader() -> Result<(), StreamError> {
        let input = b"GET / HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\n".to_vec();
        let mut c = Cursor::new(input);
        let mut reader = StreamReader::new(&mut c);

        let out = reader.read_line().await?;
        assert_eq!(String::from_utf8_lossy(&out), "GET / HTTP/1.1".to_string());

        let out = reader.read_line().await?;
        assert_eq!(
            String::from_utf8_lossy(&out),
            "Host: localhost:42069".to_string()
        );

        let out = reader.read_line().await?;
        assert_eq!(
            String::from_utf8_lossy(&out),
            "User-Agent: curl/7.81.0".to_string()
        );

        let out = reader.read_line().await?;
        assert_eq!(String::from_utf8_lossy(&out), "Accept: */*".to_string());

        let out = reader.read_line().await?;
        assert_eq!(String::from_utf8_lossy(&out), "".to_string());

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_reader_line_longer_than_buf() -> Result<(), StreamError> {
        let len = {
            let mut c = Cursor::new("");
            let reader = StreamReader::new(&mut c);
            reader.buf.len()
        };
        let mut input = vec![0u8; len + 3];
        input.extend_from_slice(b"\r\n");
        let mut c = Cursor::new(input.clone());
        let mut reader = StreamReader::new(&mut c);

        let out = reader.read_line().await?;
        assert_eq!(out, input[..input.len().saturating_sub(2)]);

        Ok(())
    }
}
