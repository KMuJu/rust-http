#[cfg(test)]
pub mod batch_reader {
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    use tokio::io::AsyncRead;
    use tokio::io::AsyncReadExt;

    pub struct BatchReader {
        src: Vec<u8>,
        batch_size: usize,
        pos: usize,
    }

    impl BatchReader {
        pub fn new(destination: Vec<u8>, batch_size: usize) -> BatchReader {
            BatchReader {
                src: destination,
                batch_size,
                pos: 0,
            }
        }
    }

    impl AsyncRead for BatchReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if self.pos >= self.src.len() {
                return Poll::Ready(Ok(())); // EOF
            }

            let remaining = &self.src[self.pos..];
            let size = remaining.len().min(buf.remaining()).min(self.batch_size);

            buf.put_slice(&remaining[..size]);
            self.pos += size;

            Poll::Ready(Ok(()))
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;

        #[tokio::test]
        async fn test_request_parser() -> std::io::Result<()> {
            let input = b"aaabbbccc".to_vec();
            let mut reader = BatchReader::new(input, 3);
            let mut buf = [0u8; 3];

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"aaa");

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"bbb");

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"ccc");

            Ok(())
        }
        #[tokio::test]
        async fn test_request_parser_1() -> std::io::Result<()> {
            let input = b"aaabbbc".to_vec();
            let mut reader = BatchReader::new(input, 3);
            let mut buf = [0u8; 2];

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"aa");

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"ab");

            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"bb");

            let mut buf = [0u8; 2];
            let n = reader.read(&mut buf).await?;
            assert_eq!(n, 1);
            assert_eq!(&buf, &[b'c', 0]);

            Ok(())
        }
    }
}
