use std::sync::Arc;
use log::{Level, warn};
use tokio::net::TcpListener;
use rusty_socks::context::Context;
use rusty_socks::stream::Stream;
use rusty_socks::states::State;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(Level::Debug).unwrap();
    let mut listener = TcpListener::bind("127.0.0.1:8080").await?;
    let context = Arc::new(Context::new());
    loop {
        let (stream, _) = listener.accept().await?;
        let context = Arc::clone(&context);
        tokio::spawn(async move {
            let stream = Stream::buffered(stream);
            let mut state = State::new(stream);
            loop {
                let result = state.process(&context).await;
                if result.is_err() {
                    warn!("Stream finished with error: {:?}", result.err().unwrap());
                    break;
                }
                state = result.unwrap();
                if state.is_finished() {
                    break;
                }
            }
        });
    }
}
