use tokio::io::{self, AsyncReadExt};

pub struct StreamReader<R> {
    read: usize,
    buf: [u8; 2048],
    reader: R,
}

impl<R: AsyncReadExt + Unpin> StreamReader<R> {
    pub fn new(reader: R) -> Self {
        StreamReader {
            read: 0,
            buf: [0u8; 2048],
            reader,
        }
    }

    pub async fn read_line(&mut self) -> io::Result<Vec<u8>> {
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
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Unexpected EOF",
                ));
            }
            self.read = n;
        }
    }

    pub async fn read_n(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(n);
        let len = self.read.min(n);
        if self.read > 0 {
            buf.extend_from_slice(&self.buf[..len]);
            self.buf.copy_within(len.., 0);
            self.read -= len;
        }

        let remaining = n.saturating_sub(len);
        if remaining == 0 {
            return Ok(buf);
        }
        let mut b = vec![0u8; remaining];
        let mut read = 0;
        while read < remaining {
            let n = self.reader.read(&mut b[read..]).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "Unexpected EOF",
                ));
            }
            read += n;
        }

        // copies into the buf from self.read
        // if self.read is longer than n or buf len then it will return earlier
        buf.extend_from_slice(&b);

        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    use super::*;

    #[tokio::test]
    async fn test_stream_reader() -> io::Result<()> {
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
    async fn test_stream_reader_line_longer_than_buf() -> io::Result<()> {
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

    #[tokio::test]
    async fn test_read_n() -> io::Result<()> {
        let mut c = Cursor::new(b"abab");
        let mut reader = StreamReader::new(&mut c);

        let buf = reader.read_n(4).await?;

        assert_eq!(buf.len(), 4);
        assert_eq!(String::from_utf8_lossy(&buf[..4]), "abab");

        Ok(())
    }

    #[tokio::test]
    async fn test_read_n_after_read_line() -> io::Result<()> {
        let mut c = Cursor::new(b"aa\r\nbbb");
        let mut reader = StreamReader::new(&mut c);

        reader.read_line().await?;

        let buf = reader.read_n(3).await?;

        assert_eq!(buf.len(), 3);
        assert_eq!(String::from_utf8_lossy(&buf[..3]), "bbb");

        Ok(())
    }

    #[tokio::test]
    async fn test_read_n_multiple() -> io::Result<()> {
        let mut c = Cursor::new(b"aaaabbb");
        let mut reader = StreamReader::new(&mut c);

        let read = 4;
        let buf = reader.read_n(read).await?;
        assert_eq!(buf.len(), read);
        assert_eq!(String::from_utf8_lossy(&buf[..read]), "aaaa");

        let read = 3;
        let buf = reader.read_n(read).await?;
        assert_eq!(buf.len(), read);
        assert_eq!(String::from_utf8_lossy(&buf[..read]), "bbb");

        Ok(())
    }
}
