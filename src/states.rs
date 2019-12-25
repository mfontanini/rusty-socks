use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use log::info;
use futures::try_join;
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::io::{split, BufReader, BufWriter, ReadHalf, WriteHalf};
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
        if request.version != 5 {
            return Err(Error::MalformedMessage(format!("Unsupported socks version {}", request.version)));
        }
        if request.methods.len() == 0 {
            return Err(Error::MalformedMessage(String::from("No methods provided")));
        }
        // TODO: pick one based on context
        let method = request.methods[0];
        
        info!("Received new client using auth method {}", method);
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
        if request.version != 5 {
            return Err(Error::MalformedMessage(String::from("Invalid socks version")))
        }
        let endpoint = match request.address {
            Address::Ip(address) => {
                SocketAddr::new(address, request.port)
            }
        };
        info!("Establishing connection with {}", endpoint);
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
            BufWriter::new(writer)
        );
        Ok(Self::Proxying(client_stream, Box::new(output_stream)))
    }

    async fn do_proxy(
        client_stream: Box<dyn ReadWriteStream>,
        output_stream: Box<dyn ReadWriteStream>
    )
        -> Result<Self, Error>
    {
        let (client_reader, client_writer) = split(client_stream);
        let (output_reader, output_writer) = split(output_stream);
        let mut client_proxier = Proxier::new(client_reader, output_writer);
        let mut output_proxier = Proxier::new(output_reader, client_writer);
        // We don't really care what happened, we're done anyway
        let _result = try_join!(
            client_proxier.run(),
            output_proxier.run()
        );
        Ok(Self::Finished)
    }
}

struct Proxier {
    reader: ReadHalf<Box<dyn ReadWriteStream>>,
    writer: WriteHalf<Box<dyn ReadWriteStream>>,
    buffer: [u8; 4096]
}

impl Proxier {
    fn new(
        reader: ReadHalf<Box<dyn ReadWriteStream>>,
        writer: WriteHalf<Box<dyn ReadWriteStream>>
    ) -> Proxier
    {
        Proxier {
            reader,
            writer,
            buffer: [0; 4096]
        }
    }

    async fn run(&mut self) -> Result<(), Error> {
        loop {
            let bytes_read = self.reader.read(&mut self.buffer).await?;
            if bytes_read == 0 {
                return Err(Error::Finished);
            }
            self.writer.write_all(&self.buffer[0..bytes_read]).await?;
            self.writer.flush().await?;
        }
    } 
}
