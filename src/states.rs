use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use log::info;
use futures::try_join;
use tokio::net::TcpStream;
use tokio::io::{copy, split, BufReader, BufWriter};
use crate::context::Context;
use crate::error::Error;
use crate::messages::*;
use crate::stream::ReadWriteStream;
use crate::stream::MergeIO;

pub enum State
{
    AwaitingHello(Box<dyn ReadWriteStream>),
    AwaitingClientRequest(Box<dyn ReadWriteStream>),
    Proxying(Box<dyn ReadWriteStream>, Box<dyn ReadWriteStream>),
    Finished
}

impl State {
    const VALID_VERSIONS: [u8; 2] = [4, 5];

    pub fn new(stream: Box<dyn ReadWriteStream>) -> State {
        State::AwaitingHello(stream)
    }

    pub fn is_finished(&self) -> bool {
        match self {
            State::Finished => true,
            _ => false
        }
    }

    pub async fn process(self, context: &Context) -> Result<Self, Error>
    {
        match self {
            State::AwaitingHello(client_stream) => {
                State::process_await_hello(client_stream, &context).await
            },
            State::AwaitingClientRequest(client_stream) => {
                State::process_await_client_request(client_stream, &context).await
            },
            State::Proxying(client_stream, output_stream) => {
                State::do_proxy(client_stream, output_stream).await
            },
            State::Finished => {
                Err(Error::Finished)
            }
        }
        
    }

    async fn process_await_hello(stream: Box<dyn ReadWriteStream>, _context: &Context)
        -> Result<Self, Error>
    {
        let (request, stream) = HelloRequest::new(stream).await?;
        if !State::VALID_VERSIONS.contains(&request.version) {
            return Err(Error::MalformedMessage(format!("Unsupported socks version {}", request.version)));
        }
        if request.methods.len() == 0 {
            return Err(Error::MalformedMessage(String::from("No methods provided")));
        }
        // TODO: pick one based on context
        let method = request.methods[0];
        info!("Received new client using method {}", method);
        let response = HelloResponse::new(request.version, method);
        let stream = response.write(stream).await?;
        Ok(State::AwaitingClientRequest(stream)) 
    }

    async fn process_await_client_request(
        client_stream: Box<dyn ReadWriteStream>,
        _context: &Context
    )
        -> Result<Self, Error>
    {
        let (request, client_stream) = ClientRequest::new(client_stream).await?;
        if !State::VALID_VERSIONS.contains(&request.version) {
            return Err(Error::MalformedMessage(String::from("Invalid socks version")))
        }
        let endpoint = match request.address {
            Address::Ip(address) => {
                SocketAddr::new(address, request.port)
            }
        };
        let output_stream = TcpStream::connect(endpoint).await?;
        let response = RequestResponse::new(
            request.version,
            ResponseCode::Success,
            Address::Ip(IpAddr::V4(Ipv4Addr::from(0))),
            0 // Port?
        );
        let client_stream = response.write(client_stream).await?;

        let (reader, writer) = split(output_stream);
        let output_stream = MergeIO::new(
            BufReader::new(reader),
            writer
            //BufWriter::new(writer)
        );
        Ok(Self::Proxying(client_stream, Box::new(output_stream)))
    }

    async fn do_proxy(
        client_stream: Box<dyn ReadWriteStream>,
        output_stream: Box<dyn ReadWriteStream>
    )
        -> Result<Self, Error>
    {
        // Split and proxy them
        let (mut client_read, mut client_write) = split(client_stream);
        let (mut output_read, mut output_write) = split(output_stream);
        let client_to_output = copy(&mut client_read, &mut output_write);
        let output_to_client = copy(&mut output_read, &mut client_write);
        try_join!(client_to_output, output_to_client)?;
        Ok(Self::Finished)
    }
}
