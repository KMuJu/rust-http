use std::io;

use tokio::io::AsyncWriteExt;

use crate::message::{Headers, Method, RequestLine};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::version::HttpVersion;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_write_to() -> io::Result<()> {
        let mut request = Request {
            line: RequestLine::from_parts(Method::Get, "/".to_string(), HttpVersion::from((1, 1))),
            headers: Headers::new(),
            body: Vec::new(),
        };
        let mut w = Vec::new();
        request.write_to(&mut w).await?;

        assert_eq!(String::from_utf8_lossy(&w), "GET / HTTP/1.1\r\n\r\n");
        Ok(())
    }
}
