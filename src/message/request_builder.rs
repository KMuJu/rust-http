use crate::message::{Headers, Method, Request, RequestLine};

pub struct RequestBuilder {
    request_line: RequestLine,
    headers: Headers,
    body: Vec<u8>,
}

impl RequestBuilder {
    pub fn new(method: Method, url: impl Into<String>) -> RequestBuilder {
        RequestBuilder {
            request_line: RequestLine::new(method, url.into(), "1.1".to_string()),
            headers: Headers::new(),
            body: Vec::new(),
        }
    }

    pub fn header<K, V>(&mut self, name: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.add(name, value);
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn build(self) -> Request {
        Request {
            line: self.request_line,
            headers: self.headers,
            body: self.body,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_builder() {
        let mut builder = RequestBuilder::new(Method::Get, "/");
        builder.header("AA", "BB");
        let request = builder.build();

        assert_eq!(request.body.len(), 0);
        assert_eq!(request.line.method, Method::Get);
        assert_eq!(request.line.url, "/".to_string());
        assert_eq!(request.line.version, "1.1".to_string());
        assert_eq!(request.headers.get("AA"), Some(&"BB".to_string()));
    }
}
