use crate::error::Error;
use async_trait::async_trait;
use num_traits::FromPrimitive;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::*;

// Common types

#[derive(Primitive, PartialEq, Debug, Copy, Clone)]
pub enum AuthenticationMethod {
    NoAuthentication = 0,
    UsernamePassword = 2,
}

impl fmt::Display for AuthenticationMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\"{:?}\"", self)
    }
}

#[derive(Debug, PartialEq, Primitive)]
pub enum Command {
    Connect = 1,
}

#[derive(Debug, PartialEq)]
pub enum Address {
    Ip(IpAddr),
    Domain(String),
}

#[derive(Primitive, PartialEq, Debug)]
pub enum AddressType {
    Ipv4 = 1,
    Domain = 3,
    Ipv6 = 4,
}

#[derive(Copy, Clone)]
pub enum ResponseCode {
    Success = 0,
    GeneralFailure = 1,
}

#[derive(Copy, Clone, Debug)]
pub enum AuthStatusCode {
    Success = 0,
    Failure = 1,
}

// Messages

pub struct HelloRequest {
    pub version: u8,
    pub methods: Vec<AuthenticationMethod>,
}

pub struct HelloResponse {
    pub version: u8,
    pub method: AuthenticationMethod,
}

pub struct AuthRequest {
    pub version: u8,
    pub username: String,
    pub password: String,
}

pub struct AuthResponse {
    pub version: u8,
    pub status: AuthStatusCode,
}

pub struct ClientRequest {
    pub version: u8,
    pub command: Command,
    pub address: Address,
    pub port: u16,
}

pub struct RequestResponse {
    pub version: u8,
    pub response_code: ResponseCode,
    pub bind_address: Address,
    pub port: u16,
}

// Traits

#[async_trait]
pub trait Parseable {
    async fn new<T>(input: &mut T) -> Result<Self, Error>
    where
        Self: Sized,
        T: AsyncRead + Send + Unpin;
}

#[async_trait]
pub trait Writeable {
    async fn write<T>(&self, output: &mut T) -> Result<(), Error>
    where
        T: AsyncWrite + Send + Unpin;
}

// Allow reading strings from an AsyncRead

#[async_trait]
trait ReadString {
    async fn read_string(&mut self) -> Result<String, Error>;
}

#[async_trait]
impl<T: AsyncRead + Send + Unpin> ReadString for T {
    async fn read_string(&mut self) -> Result<String, Error> {
        let length = self.read_u8().await? as usize;
        let mut domain = Vec::with_capacity(length);
        domain.resize(length, 0);
        self.read_exact(domain.as_mut_slice()).await?;
        let parsed_string = String::from_utf8(domain);
        if parsed_string.is_err() {
            return Err(Error::MalformedMessage("Invalid string in stream".into()));
        }
        Ok(parsed_string.unwrap())
    }
}

// Request impls

#[async_trait]
impl Parseable for HelloRequest {
    async fn new<T>(input: &mut T) -> Result<Self, Error>
    where
        T: AsyncRead + Send + Unpin,
    {
        let version = input.read_u8().await?;
        let method_count = input.read_u8().await?;
        let mut methods = Vec::new();
        for _i in 0..method_count {
            match AuthenticationMethod::from_u8(input.read_u8().await?) {
                Some(method) => methods.push(method),
                None => return Err(Error::MalformedMessage("Unsupported method".into())),
            };
        }
        Ok(HelloRequest { version, methods })
    }
}

#[async_trait]
impl Parseable for ClientRequest {
    async fn new<T>(input: &mut T) -> Result<Self, Error>
    where
        T: AsyncRead + Send + Unpin,
    {
        let version = input.read_u8().await?;
        let command = Command::from_u8(input.read_u8().await?)
            .ok_or_else(|| Error::MalformedMessage("Unsupported command".into()))?;
        // Skip reserved byte
        input.read_u8().await?;
        let address_type = AddressType::from_u8(input.read_u8().await?)
            .ok_or_else(|| Error::MalformedMessage("Invalid address type".into()))?;
        let address = match address_type {
            AddressType::Ipv4 => {
                let addr = input.read_u32().await?;
                Address::Ip(IpAddr::V4(Ipv4Addr::from(addr)))
            }
            AddressType::Ipv6 => {
                let mut buf = [0; 16];
                input.read_exact(&mut buf).await?;
                Address::Ip(IpAddr::V6(Ipv6Addr::from(buf)))
            }
            AddressType::Domain => Address::Domain(input.read_string().await?),
        };
        let port = input.read_u16().await?;
        Ok(ClientRequest {
            version,
            command,
            address,
            port,
        })
    }
}

#[async_trait]
impl Parseable for AuthRequest {
    async fn new<T>(input: &mut T) -> Result<Self, Error>
    where
        T: AsyncRead + Send + Unpin,
    {
        let version = input.read_u8().await?;
        if version != 1 {
            return Err(Error::Generic("Unsupported auth version".into()));
        }
        let username = input.read_string().await?;
        let password = input.read_string().await?;
        Ok(AuthRequest {
            version,
            username,
            password,
        })
    }
}

// Response impls

impl HelloResponse {
    pub fn new(version: u8, method: AuthenticationMethod) -> Self {
        HelloResponse { version, method }
    }
}

#[async_trait]
impl Writeable for HelloResponse {
    async fn write<T>(&self, output: &mut T) -> Result<(), Error>
    where
        T: AsyncWrite + Send + Unpin,
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.method as u8).await?;
        output.flush().await?;
        Ok(())
    }
}

impl RequestResponse {
    pub fn new(
        version: u8,
        response_code: ResponseCode,
        bind_address: Address,
        port: u16,
    ) -> RequestResponse {
        RequestResponse {
            version,
            response_code,
            bind_address,
            port,
        }
    }
}

#[async_trait]
impl Writeable for RequestResponse {
    async fn write<T>(&self, output: &mut T) -> Result<(), Error>
    where
        T: AsyncWrite + Send + Unpin,
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.response_code as u8).await?;
        // Reserved byte
        output.write_u8(0).await?;
        match self.bind_address {
            Address::Ip(IpAddr::V4(address)) => {
                output.write_u8(AddressType::Ipv4 as u8).await?;
                output.write_all(&address.octets()).await?;
            }
            Address::Ip(IpAddr::V6(address)) => {
                output.write_u8(AddressType::Ipv6 as u8).await?;
                output.write_all(&address.octets()).await?;
            }
            Address::Domain(ref _domain) => {
                panic!("Domain used for bind address");
            }
        };
        output.write_u16(self.port).await?;
        output.flush().await?;
        Ok(())
    }
}

impl AuthResponse {
    pub fn new(version: u8, status: AuthStatusCode) -> Self {
        AuthResponse { version, status }
    }
}

#[async_trait]
impl Writeable for AuthResponse {
    async fn write<T>(&self, output: &mut T) -> Result<(), Error>
    where
        T: AsyncWrite + Send + Unpin,
    {
        output.write_u8(self.version).await?;
        output.write_u8(self.status as u8).await?;
        output.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_await_test::async_test;
    use tokio::io::{BufReader, BufWriter};

    async fn make_message<T: Parseable>(buffer: &[u8]) -> T {
        let mut cursor = BufReader::new(buffer);
        let message = T::new(&mut cursor).await.unwrap();
        message
    }

    async fn expect_serialization<T: Writeable>(message: &T, expected: &[u8]) {
        let mut stream = BufWriter::new(Vec::new());
        message.write(&mut stream).await.unwrap();
        assert_eq!(stream.get_ref().as_slice(), expected);
    }

    #[async_test]
    async fn parse_hello_request() {
        let message = make_message::<HelloRequest>(&[5, 2, 0, 2]).await;
        assert_eq!(message.version, 5);
        assert_eq!(
            message.methods,
            vec!(
                AuthenticationMethod::NoAuthentication,
                AuthenticationMethod::UsernamePassword
            )
        );
    }

    #[async_test]
    async fn parse_auth_request() {
        let message = make_message::<AuthRequest>(&[1, 3, 102, 111, 111, 3, 98, 97, 114]).await;
        assert_eq!(message.version, 1);
        assert_eq!(message.username, "foo");
        assert_eq!(message.password, "bar");
    }

    #[async_test]
    async fn parse_client_request_connect_ipv4() {
        let message = make_message::<ClientRequest>(&[5, 1, 0, 1, 1, 2, 3, 4, 31, 144]).await;
        assert_eq!(message.version, 5);
        assert_eq!(message.command, Command::Connect);
        assert_eq!(message.address, Address::Ip("1.2.3.4".parse().unwrap()));
        assert_eq!(message.port, 8080);
    }

    #[async_test]
    async fn parse_client_request_connect_ipv6() {
        let message = make_message::<ClientRequest>(&[
            5, 1, 0, 4, 222, 173, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 190, 239, 31, 144,
        ])
        .await;
        assert_eq!(message.version, 5);
        assert_eq!(message.command, Command::Connect);
        assert_eq!(message.address, Address::Ip("dead::beef".parse().unwrap()));
        assert_eq!(message.port, 8080);
    }

    #[async_test]
    async fn parse_client_request_connect_domain() {
        let message = make_message::<ClientRequest>(&[
            5, 1, 0, 3, 7, 102, 111, 111, 46, 99, 111, 109, 31, 144,
        ])
        .await;
        assert_eq!(message.version, 5);
        assert_eq!(message.command, Command::Connect);
        assert_eq!(message.address, Address::Domain("foo.com".into()));
        assert_eq!(message.port, 8080);
    }

    #[async_test]
    async fn serialize_hello_reply() {
        let message = HelloResponse::new(1, AuthenticationMethod::NoAuthentication);
        expect_serialization(&message, &[1, 0]).await;
    }

    #[async_test]
    async fn serialize_auth_response() {
        let message = AuthResponse::new(1, AuthStatusCode::Success);
        expect_serialization(&message, &[1, 0]).await;
    }

    #[async_test]
    async fn serialize_request_response_ipv4() {
        let message = RequestResponse::new(
            1,
            ResponseCode::Success,
            Address::Ip("1.2.3.4".parse().unwrap()),
            8080,
        );
        expect_serialization(&message, &[1, 0, 0, 1, 1, 2, 3, 4, 31, 144]).await;
    }

    #[async_test]
    async fn serialize_request_response_ipv6() {
        let message = RequestResponse::new(
            1,
            ResponseCode::Success,
            Address::Ip("dead::beef".parse().unwrap()),
            8080,
        );
        expect_serialization(
            &message,
            &[
                1, 0, 0, 4, 222, 173, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 190, 239, 31, 144,
            ],
        )
        .await;
    }
}
