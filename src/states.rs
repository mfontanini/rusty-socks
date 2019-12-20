use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use log::info;
use tokio::net::TcpStream;
use crate::context::Context;
use crate::error::Error;
use crate::messages::*;
use crate::stream::ReadWriteStream;
use crate::stream::MergeIO;

pub enum State<Stream>
where
    Stream: ReadWriteStream
{
    AwaitingHello(Stream),
    AwaitingClientRequest(Stream),
    Proxying(Stream, Stream),
    Finished
}

impl<Stream> State<Stream>
where Stream: ReadWriteStream
{
    pub fn new(stream: Stream) -> State<Stream> {
        State::AwaitingHello(stream)
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

    async fn process_await_hello(stream: Stream, _context: &Context)
        -> Result<Self, Error>
    {
        let (request, stream) = HelloRequest::new(stream).await?;
        if request.version != 5 {
            return Err(Error::MalformedMessage(String::from("Unsupported socks version")));
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

    async fn process_await_client_request(client_stream: Stream, _context: &Context)
        -> Result<Self, Error>
    {
        let (request, client_stream) = ClientRequest::new(client_stream).await?;
        if request.version != 5 {
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

        let (reader, writer) = output_stream.split();
        Ok(Self::Proxying(client_stream, MergeIO::buffered(reader, writer)))
    }

    async fn do_proxy(client_stream: Stream, output_stream: Stream)
        -> Result<Self, Error>
    {
        // TODO: nope
        Err(Error::Finished)
    }
}
