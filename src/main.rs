use tokio::net::TcpListener;
use tokio::io::{BufReader, BufWriter, split};
use rusty_socks::context::Context;
use rusty_socks::stream::MergeIO;
use rusty_socks::states::{State};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            let context = Context::new();
            let (reader, writer) = split(stream);
            let stream = MergeIO::new(
                BufReader::new(reader),
                writer
                //BufWriter::new(writer)
            );
            let mut state = State::new(Box::new(stream));
            loop {
               state = state.process(&context).await.unwrap();
            }
        });
    }
}
