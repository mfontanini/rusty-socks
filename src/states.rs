use tokio::io::{AsyncRead, AsyncWrite};
use crate::messages::{HelloRequest, HelloResponse, Parseable, Writeable};
use crate::error::Error;
use crate::context::Context;

pub enum State<Stream>
where Stream: AsyncRead + AsyncWrite + Send + Unpin
{
    AwaitingHelloRequest(Stream),
    SentHelloResponse(Stream),
    Finished
}

impl<Stream> State<Stream>
where Stream: AsyncRead + AsyncWrite + Send + Unpin
{
    pub fn new(stream: Stream) -> State<Stream> {
        State::AwaitingHelloRequest(stream)
    }

    pub async fn process(self, context: &Context) -> Result<Self, Error>
    {
        match self {
            State::AwaitingHelloRequest(client_stream) => {
                State::process_await_hello_request(client_stream, &context).await
            },
            State::SentHelloResponse(client_stream) => {
                Ok(State::Finished)
            },
            State::Finished => {
                Err(Error::Finished)
            }
        }
        
    }

    async fn process_await_hello_request(stream: Stream, context: &Context)
        -> Result<Self, Error>
    {
        let (request, stream) = HelloRequest::new(stream).await?;
        if request.methods.len() == 0 {
            return Err(Error::MalformedMessage(String::from("No methods provided")));
        }
        let response = HelloResponse::new(request.version, request.methods[0]);
        let stream = response.write(stream).await?;
        Ok(State::SentHelloResponse(stream)) 
    }
}
