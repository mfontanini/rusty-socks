use std::env;
use std::fs;
use std::process::exit;
use std::sync::Arc;
use log::{Level, info, warn};
use tokio::net::TcpListener;
use serde::Deserialize;
use rusty_socks::context::{Context, Credentials};
use rusty_socks::stream::Stream;
use rusty_socks::states::State;

#[derive(Deserialize)]
struct Config {
    endpoint: String,
    credentials: Option<ConfigCredentials>
}

#[derive(Deserialize)]
struct ConfigCredentials {
    username: String,
    password: String
}

fn load_config(filename: &str) -> Config {
    let config_contents = fs::read_to_string(filename);
    if config_contents.is_err() {
        eprintln!("Failed to load config file");
        exit(1);
    }
    match toml::from_str(&config_contents.unwrap()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to parse config: {:}", e);
            exit(1)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(Level::Debug).unwrap();
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {:} <toml-file-path>", args.get(0).map(|s| s.as_str()).unwrap_or("rusty-socks"));
        exit(1);
    }
    let config = load_config(&args[1]);
    let listener = TcpListener::bind(&config.endpoint).await?;
    let context = match config.credentials {
        Some(c) => {
            info!("Using credentials: {}:xxx", c.username);
            Context::with_credentials(Credentials::new(&c.username, &c.password))
        },
        None => {
            info!("Using no authentication");
            Context::new()
        }
    };
    let context = Arc::new(context);
    info!("Server running on endpoint {}", config.endpoint);
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
