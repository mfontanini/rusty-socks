use tokio::net::TcpListener;
use tokio::io::{BufReader, BufWriter};
use rusty_socks::context::Context;
use rusty_socks::stream::MergeIO;
use rusty_socks::states::{State};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (mut stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let context = Context::new();
            let (reader, writer) = stream.split();
            let stream = MergeIO::buffered(reader, writer);
            let mut state = State::new(stream);
            loop {
               state = state.process(&context).await.unwrap();
            }
        });
    }
}
