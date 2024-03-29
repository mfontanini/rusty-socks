use crate::context::Context;
use crate::error::Error;
use crate::messages::*;
use crate::stream::Stream;
use futures::try_join;
use log::{debug, info};
use std::net::{IpAddr, Ipv4Addr};
use tokio::io::{split, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;

pub enum State {
    AwaitingHello(Stream),
    AwaitingAuth(Stream),
    AwaitingClientRequest(Stream),
    Proxying(Stream, Stream),
    Finished,
}

impl State {
    pub fn new(stream: Stream) -> Self {
        State::AwaitingHello(stream)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, State::Finished)
    }

    pub async fn process(self, context: &Context) -> Result<Self, Error> {
        match self {
            State::AwaitingHello(client_stream) => {
                State::process_await_hello(client_stream, context).await
            }
            State::AwaitingAuth(client_stream) => {
                State::process_await_auth(client_stream, context).await
            }
            State::AwaitingClientRequest(client_stream) => {
                State::process_await_client_request(client_stream).await
            }
            State::Proxying(client_stream, output_stream) => {
                State::do_proxy(client_stream, output_stream).await
            }
            State::Finished => Err(Error::Finished),
        }
    }

    async fn process_await_hello(mut stream: Stream, context: &Context) -> Result<Self, Error> {
        let request = HelloRequest::new(&mut stream).await?;
        if request.version != 5 {
            return Err(Error::MalformedMessage(
                format!("Unsupported socks version {}", request.version).into(),
            ));
        }
        if request.methods.is_empty() {
            return Err(Error::MalformedMessage("No methods provided".into()));
        }
        let selected_method = match context.select_authentication(request.methods) {
            Some(method) => method,
            None => return Ok(State::Finished),
        };
        info!("Received new client using auth {}", selected_method);
        let response = HelloResponse::new(request.version, selected_method);
        response.write(&mut stream).await?;
        match selected_method {
            AuthenticationMethod::NoAuthentication => Ok(State::AwaitingClientRequest(stream)),
            AuthenticationMethod::UsernamePassword => Ok(State::AwaitingAuth(stream)),
        }
    }

    async fn process_await_auth(mut stream: Stream, context: &Context) -> Result<Self, Error> {
        let request = AuthRequest::new(&mut stream).await?;
        let status = match context.authenticate(&request.username, &request.password) {
            true => AuthStatusCode::Success,
            false => AuthStatusCode::Failure,
        };
        debug!("Authentication request finished with status: {:?}", status);
        let response = AuthResponse::new(request.version, status);
        response.write(&mut stream).await?;
        Ok(State::AwaitingClientRequest(stream))
    }

    async fn process_await_client_request(mut client_stream: Stream) -> Result<Self, Error> {
        let request = ClientRequest::new(&mut client_stream).await?;
        if request.version != 5 {
            return Err(Error::MalformedMessage("Invalid socks version".into()));
        }
        let output_stream = match request.address {
            Address::Ip(address) => {
                let endpoint = (address, request.port);
                info!("Establishing connection with {:?}", endpoint);
                TcpStream::connect(endpoint).await
            }
            Address::Domain(ref domain) => {
                let endpoint = (domain.as_str(), request.port);
                info!("Establishing connection with {:?}", endpoint);
                TcpStream::connect(endpoint).await
            }
        }?;
        let response = RequestResponse::new(
            request.version,
            ResponseCode::Success,
            Address::Ip(IpAddr::V4(Ipv4Addr::from(0))),
            0, // Port?
        );
        response.write(&mut client_stream).await?;
        Ok(Self::Proxying(
            client_stream,
            Stream::unbuffered(output_stream),
        ))
    }

    async fn do_proxy(client_stream: Stream, output_stream: Stream) -> Result<Self, Error> {
        let (client_reader, client_writer) = split(client_stream.into_unbuffered());
        let (output_reader, output_writer) = split(output_stream.into_unbuffered());
        let mut client_proxier = Proxier::new(client_reader, output_writer);
        let mut output_proxier = Proxier::new(output_reader, client_writer);
        // We don't really care what happened, we're done anyway
        let _result = try_join!(client_proxier.run(), output_proxier.run());
        Ok(Self::Finished)
    }
}

struct Proxier {
    reader: ReadHalf<Stream>,
    writer: WriteHalf<Stream>,
}

impl Proxier {
    fn new(reader: ReadHalf<Stream>, writer: WriteHalf<Stream>) -> Self {
        Proxier { reader, writer }
    }

    async fn run(&mut self) -> Result<(), Error> {
        let mut buffer = [0; 4096];
        loop {
            let bytes_read = self.reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                return Err(Error::Finished);
            }
            self.writer.write_all(&buffer[0..bytes_read]).await?;
            self.writer.flush().await?;
        }
    }
}
