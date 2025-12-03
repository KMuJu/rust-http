use crate::message::error::RequestLineError;

#[derive(Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
}

impl Method {
    pub fn parse(bytes: &[u8]) -> Result<Method, RequestLineError> {
        match bytes {
            b"GET" => Ok(Self::Get),
            b"HEAD" => Ok(Self::Head),
            b"POST" => Ok(Self::Post),
            b"PUT" => Ok(Self::Put),
            b"DELETE" => Ok(Self::Delete),
            b"CONNECT" => Ok(Self::Connect),
            b"OPTIONS" => Ok(Self::Options),
            b"TRACE" => Ok(Self::Trace),
            _ => Err(RequestLineError::InvalidMehtod),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Method::Get => "GET",
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Connect => "CONNECT",
            Method::Options => "OPTIONS",
            Method::Trace => "TRACE",
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Get => b"Get".to_vec(),
            Self::Head => b"Head".to_vec(),
            Self::Post => b"Post".to_vec(),
            Self::Put => b"Put".to_vec(),
            Self::Delete => b"Delete".to_vec(),
            Self::Connect => b"Connect".to_vec(),
            Self::Options => b"Options".to_vec(),
            Self::Trace => b"Trace".to_vec(),
        }
    }
}
