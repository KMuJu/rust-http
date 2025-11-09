#[cfg(test)]
pub mod batch_reader {
    use std::io::Read;

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

    impl Read for BatchReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pos >= self.src.len() {
                return Ok(0);
            }
            let mut size = self.batch_size;
            if size > buf.len() {
                size = buf.len();
            }
            if self.pos + size > self.src.len() {
                size = self.src.len() - self.pos;
            }

            let src = &self.src[self.pos..self.pos + size];
            buf[..size].copy_from_slice(src);

            self.pos += size;
            Ok(size)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_request_parser() -> std::io::Result<()> {
            let input = b"aaabbbccc".to_vec();
            let mut reader = BatchReader::new(input, 3);
            let mut buf = [0u8; 3];

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"aaa");

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"bbb");

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 3);
            assert_eq!(&buf, b"ccc");

            Ok(())
        }
        #[test]
        fn test_request_parser_1() -> std::io::Result<()> {
            let input = b"aaabbbc".to_vec();
            let mut reader = BatchReader::new(input, 3);
            let mut buf = [0u8; 2];

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"aa");

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"ab");

            let n = reader.read(&mut buf)?;
            assert_eq!(n, 2);
            assert_eq!(&buf, b"bb");

            let mut buf = [0u8; 2];
            let n = reader.read(&mut buf)?;
            assert_eq!(n, 1);
            assert_eq!(&buf, &[b'c', 0]);

            Ok(())
        }
    }
}
