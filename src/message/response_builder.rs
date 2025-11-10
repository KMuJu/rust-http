use std::io::Write;

use crate::{
    message::{Headers, Response, StatusCode, StatusLine},
    server::ServerError,
};

pub struct ResponseBuilder {
    status_line: StatusLine,
    headers: Headers,
    body: Vec<u8>,
}

impl ResponseBuilder {
    /// Creates a new [`ResponseBuilder`].
    /// Starts with a default response, which has:
    /// - Status code: 200
    /// - Default headers
    /// - Empty body
    pub fn new() -> ResponseBuilder {
        ResponseBuilder {
            status_line: StatusLine::new(StatusCode::Ok),
            headers: Headers::new_with_default(),
            body: Vec::new(),
        }
    }

    pub fn set_status_code(&mut self, status_code: StatusCode) -> &mut Self {
        self.status_line = StatusLine::new(status_code);
        self
    }

    /// Write a header.
    /// Will overwride old values
    pub fn add_header<K, V>(&mut self, name: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.set(name, value);
        self
    }

    pub fn add_to_body(&mut self, body: &[u8]) -> Result<&mut Self, ServerError> {
        self.body.write_all(body)?;
        Ok(self)
    }

    pub fn build(self) -> Response {
        Response {
            status_line: self.status_line,
            headers: self.headers,
            body: self.body,
        }
    }
}

impl Default for ResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_builder() {
        let mut builder = ResponseBuilder::new();
        builder
            .set_status_code(StatusCode::Ok)
            .add_header("AA", "BB");
        let response = builder.build();

        assert_eq!(response.body.len(), 0);
        assert_eq!(response.status_line.status_code, StatusCode::Ok);
        assert_eq!(response.headers.get("AA"), Some(&"BB".to_string()));
    }
}
