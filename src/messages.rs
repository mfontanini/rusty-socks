use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::*;
use async_trait::async_trait;
use num_traits::FromPrimitive;
use byteorder::{ReadBytesExt, WriteBytesExt};
use crate::error::Error;

#[derive(Primitive, PartialEq, Debug, Copy, Clone)]
pub enum Method {
    NoAuthentication = 0,
    UsernamePassword = 2
}

pub struct HelloRequest {
    pub version: u8,
    pub methods: Vec<Method>
}

pub struct HelloResponse {
    pub version: u8,
    pub method: Method
}

#[async_trait]
pub trait Parseable {
    async fn new<T>(mut input: T) -> Result<(Self, T), Error>
        where Self: Sized,
            T: AsyncRead + Send + Unpin;
}

#[async_trait]
pub trait Writeable {
    async fn write<T>(&self, mut output: T) -> Result<T, Error>
        where T: AsyncWrite + Send + Unpin;
}

#[async_trait]
impl Parseable for HelloRequest {
    async fn new<T>(mut input: T) -> Result<(HelloRequest, T), Error>
    where T: AsyncRead + Send + Unpin
    {
        let version = input.read_u8().await?;
        let method_count = input.read_u8().await?;
        let mut methods = Vec::new();
        for _i in 0..method_count {
            let method = Method::from_u8(input.read_u8().await?);
            if method.is_none() {
                return Err(Error::MalformedMessage(String::from("Unsupported method")));
            }
            methods.push(method.unwrap());
        }
        Ok((HelloRequest{version, methods}, input))
    }
}

impl HelloResponse {
    pub fn new(version: u8, method: Method) -> HelloResponse {
        HelloResponse{
            version,
            method
        }
    }
}

#[async_trait]
impl Writeable for HelloResponse {
    async fn write<T>(&self, mut output: T) -> Result<T, Error>
        where T: AsyncWrite + Send + Unpin
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.method as u8).await?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_await_test::async_test;
    use tokio::io::{BufWriter, BufReader};

    async fn make_message<T: Parseable>(buffer: &[u8]) -> T {
        let cursor = BufReader::new(buffer);
        let (message, _) = T::new(cursor).await.unwrap();
        message
    }

    async fn expect_serialization<T: Writeable>(message: &T, expected: &[u8]) {
        let mut stream = BufWriter::new(Vec::new());
        let stream = message.write(&mut stream).await.unwrap();
        assert_eq!(stream.buffer(), expected);
    }

    #[async_test]
    async fn hello_request_parse() {
        let message = make_message::<HelloRequest>(&[5, 2, 0, 2]).await;
        assert_eq!(message.version, 5);
        assert_eq!(message.methods,
                   vec!(Method::NoAuthentication, Method::UsernamePassword));
    }

    #[async_test]
    async fn hello_reply_serialize() {
        let message = HelloResponse::new(1, Method::NoAuthentication);
        expect_serialization(&message, &[1, 0]).await;
    }
}