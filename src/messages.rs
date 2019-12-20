use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::io::{AsyncRead, AsyncWrite};
use async_trait::async_trait;
use num_traits::FromPrimitive;
use crate::error::Error;
use tokio::prelude::*;
//use tokio_byteorder::{BigEndian, AsyncReadBytesExt, AsyncWriteBytesExt};

// Common types

#[derive(Primitive, PartialEq, Debug, Copy, Clone)]
pub enum Method {
    NoAuthentication = 0,
    UsernamePassword = 2
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

pub enum Command {
    Connect
}

pub enum Address {
    Ip(IpAddr)
}

#[derive(Primitive, PartialEq, Debug)]
pub enum AddressType {
    Ipv4 = 1,
    Ipv6 = 4
}

#[derive(Copy, Clone)]
pub enum ResponseCode {
    Success = 0,
    GeneralFailure = 1
}

// Messages

pub struct HelloRequest {
    pub version: u8,
    pub methods: Vec<Method>
}

pub struct HelloResponse {
    pub version: u8,
    pub method: Method
}

pub struct ClientRequest {
    pub version: u8,
    pub command: Command,
    pub address: Address,
    pub port: u16
}

pub struct RequestResponse {
    pub version: u8,
    pub response_code: ResponseCode,
    pub bind_address: Address,
    pub port: u16
}

// Traits

#[async_trait]
pub trait Parseable {
    async fn new<T>(mut input: T) -> Result<(Self, T), Error>
    where
        Self: Sized,
        T: AsyncRead + Send + Unpin;
}

#[async_trait]
pub trait Writeable {
    async fn write<T>(&self, mut output: T) -> Result<T, Error>
    where
        T: AsyncWrite + Send + Unpin;
}

// Impls

#[async_trait]
impl Parseable for HelloRequest {
    async fn new<T>(mut input: T) -> Result<(HelloRequest, T), Error>
    where
        T: AsyncRead + Send + Unpin
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

#[async_trait]
impl Parseable for ClientRequest {
    async fn new<T>(mut input: T) -> Result<(ClientRequest, T), Error>
    where
        T: AsyncRead + Send + Unpin
    {
        const COMMAND_CONNECT: u8 = 1;

        let version = input.read_u8().await?;
        let command = match input.read_u8().await? {
            COMMAND_CONNECT => Ok(Command::Connect),
            _ => Err(Error::MalformedMessage(String::from("Unsupported command")))
        }?;
        // Skip reserved byte
        input.read_u8().await?;
        let address_type = AddressType::from_u8(input.read_u8().await?);
        if address_type.is_none() {
            return Err(Error::MalformedMessage(String::from("Invalid address type")));
        }
        let address = match address_type.unwrap() {
            AddressType::Ipv4 => {
                let addr = input.read_u32().await?;
                Address::Ip(IpAddr::V4(Ipv4Addr::from(addr)))
            },
            AddressType::Ipv6 => {
                let mut buf = [0; 16];
                input.read_exact(&mut buf).await?;
                Address::Ip(IpAddr::V6(Ipv6Addr::from(buf)))  
            }
        };
        let port = input.read_u16().await?;
        Ok((ClientRequest{version, command, address, port}, input))
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
    where
        T: AsyncWrite + Send + Unpin
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.method as u8).await?;
        Ok(output)
    }
}

impl RequestResponse {
    pub fn new(
        version: u8,
        response_code: ResponseCode,
        bind_address: Address,
        port: u16)
        -> RequestResponse
    {
        RequestResponse{
            version,
            response_code,
            bind_address,
            port
        }
    }
}

#[async_trait]
impl Writeable for RequestResponse {
    async fn write<T>(&self, mut output: T) -> Result<T, Error>
    where
        T: AsyncWrite + Send + Unpin
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.response_code as u8).await?;
        // Reserved byte
        output.write_u8(0).await?;
        match self.bind_address {
            Address::Ip(IpAddr::V4(address)) => {
                output.write_u8(AddressType::Ipv4 as u8).await?;
                output.write_all(&address.octets()).await?;
            },
            Address::Ip(IpAddr::V6(address)) => {
                output.write_u8(AddressType::Ipv6 as u8).await?;
                output.write_all(&address.octets()).await?;
            }
        };
        output.write_u16(self.port).await?;
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
