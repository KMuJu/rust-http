use std::{
    collections::HashMap,
    io::{self, Write},
};

use crate::message::error::HeadersError;

#[derive(Debug)]
pub struct Headers(HashMap<String, String>);

fn is_valid_token(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| {
        matches!(b, b'A'..=b'Z'
        | b'a'..=b'z'
        | b'0'..=b'9'
        | b'!'
        | b'#'
        | b'$'
        | b'%'
        | b'&'
        | b'\''
        | b'*'
        | b'+'
        | b'-'
        | b'.'
        | b'^'
        | b'_'
        | b'`'
        | b'|'
        | b'~')
    })
}

fn is_valid_field_value(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| match b {
        0x09 | 0x20 => true, // HTAB or SP
        0x21..=0x7E => true, // VCHAR
        0x80..=0xFF => true, // obs-text
        _ => false,
    })
}
const CRLF: &[u8; 2] = b"\r\n";

impl Headers {
    pub fn new() -> Headers {
        Headers(HashMap::new())
    }

    pub fn add_default(&mut self) {
        self.set("connection".to_string(), "close".to_string()); // TODO: Implement keep alive
    }

    pub fn add<K, V>(&mut self, name: K, value: V) -> Option<String>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let name = name.into().to_lowercase();
        let value = value.into().to_string();
        if let Some(old) = self.0.get(&name) {
            let new = format!("{},{}", old, value);
            self.0.insert(name, new)
        } else {
            self.0.insert(name, value)
        }
    }

    pub fn set<K, V>(&mut self, name: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        let name = name.into().to_lowercase();
        let value = value.into().to_string();
        self.0.insert(name, value);
    }

    pub fn remove<K>(&mut self, name: K)
    where
        K: Into<String>,
    {
        let name = name.into().to_lowercase();
        self.0.remove(&name);
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.0.get(&name.to_lowercase())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn parse(&mut self, bytes: &[u8]) -> Result<usize, HeadersError> {
        let end_of_line = bytes.windows(CRLF.len()).position(|w| w == CRLF);
        let Some(end) = end_of_line else {
            return Ok(0);
        };
        if end == 0 {
            return Ok(CRLF.len());
        }
        let current_data = &bytes[..end];

        let parts = current_data
            .splitn(2, |&b| b == b':')
            .collect::<Vec<&[u8]>>();

        if parts.len() != 2 {
            return Err(HeadersError::MalformedHeader);
        }

        let name_bytes = parts[0];
        let value_bytes = parts[1].trim_ascii();
        if !is_valid_token(name_bytes) || !is_valid_field_value(value_bytes) {
            return Err(HeadersError::MalformedHeader);
        }
        let name = String::from_utf8_lossy(name_bytes).into_owned();
        let value = String::from_utf8_lossy(value_bytes).into_owned();

        self.add(&name, &value);

        Ok(end + CRLF.len())
    }

    pub fn write_to<W: Write>(&self, mut w: W) -> Result<(), io::Error> {
        if self.0.is_empty() {
            return Ok(());
        }
        // TODO: Consider switching to BTreeMap
        let mut keys: Vec<_> = self.0.keys().collect();
        keys.sort();

        for key in keys {
            let value = &self.0[key];
            write!(w, "{}: {}\r\n", key, value)?;
        }
        w.write_all(b"\r\n")?;
        Ok(())
    }
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_header_parse() -> Result<(), HeadersError> {
        let input = b"Host: localhost:42069".to_vec();
        let mut header = Headers::new();
        let n = header.parse(&input)?;
        assert_eq!(n, 0);

        let input = b"\r\n".to_vec();
        let mut header = Headers::new();
        let n = header.parse(&input)?;
        assert_eq!(n, 2);

        let input = b"Host: localhost:42069\r\n".to_vec();
        let mut header = Headers::new();
        let n = header.parse(&input)?;
        assert_eq!(header.get("Host"), Some(&"localhost:42069".to_string()));
        assert_eq!(header.get("host"), Some(&"localhost:42069".to_string()));
        assert_eq!(n, 23);

        let input = b"Host : localhost:42069\r\n".to_vec();
        let mut header = Headers::new();
        let res = header.parse(&input);
        assert!(res.is_err());

        let mut input = b"Host : localhost:42069\r\n".to_vec();
        input[0] = 1; // Invalid field value byte
        let mut header = Headers::new();
        let res = header.parse(&input);
        assert!(res.is_err());

        Ok(())
    }

    #[test]
    fn test_write_to() -> io::Result<()> {
        let mut buf = Vec::new();
        let mut headers = Headers::new();
        headers.add("a", "b");
        headers.write_to(&mut buf)?;
        assert_eq!(buf, b"a: b\r\n\r\n");

        buf = Vec::new();
        headers.add("c", "d");
        headers.write_to(&mut buf)?;
        assert_eq!(buf, b"a: b\r\nc: d\r\n\r\n");

        Ok(())
    }
}
