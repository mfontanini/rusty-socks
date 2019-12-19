use tokio::net::TcpListener;
use tokio::prelude::*;
use rusty_socks::context::Context;
use rusty_socks::stream::MergeIO;
use rusty_socks::states::{State};
use std::io;
use tokio_io::io::AllowStdIo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let context = Context::new();
            let (input, output) = socket.split();
            let stream = MergeIO::new(
                input,
                output
            );
            let mut state = State::new(stream);
            loop {
               state = state.process(&context).await.unwrap();
            }
        });
    }
}
