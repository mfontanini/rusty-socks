use std::io;
use tokio::io::{
    split,
    AsyncRead,
    AsyncWrite,
    BufReader,
    BufWriter,
    ReadBuf,
    ReadHalf,
    WriteHalf,
};
use tokio::net::TcpStream;
use std::pin::Pin;
use std::task::{Context, Poll};

enum StreamType {
    Tcp(ReadHalf<TcpStream>, WriteHalf<TcpStream>),
    BufferedTcp(BufReader<ReadHalf<TcpStream>>, BufWriter<WriteHalf<TcpStream>>)
}

pub struct Stream {
    stream_type: StreamType
}

impl Stream {
    pub fn unbuffered(stream: TcpStream) -> Self {
        let (reader, writer) = split(stream);
        Stream{
            stream_type: StreamType::Tcp(reader, writer)
        }
    }

    pub fn buffered(stream: TcpStream) -> Self {
        let (reader, writer) = split(stream);
        Stream{
            stream_type: StreamType::BufferedTcp(BufReader::new(reader), BufWriter::new(writer))
        }
    }

    pub fn into_unbuffered(self) -> Self {
        let (reader, writer) = match self.stream_type {
            StreamType::Tcp(reader, writer) => (reader, writer),
            StreamType::BufferedTcp(reader, writer) => (reader.into_inner(), writer.into_inner())
        };
        Stream{
            stream_type: StreamType::Tcp(reader, writer)
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>)
        -> Poll<io::Result<()>>
    {
        match self.get_mut().stream_type {
            StreamType::Tcp(ref mut reader, _) => {
                AsyncRead::poll_read(Pin::new(reader), cx, buf)
            },
            StreamType::BufferedTcp(ref mut reader, _) => {
                AsyncRead::poll_read(Pin::new(reader), cx, buf)
            }
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8])
        -> Poll<io::Result<usize>>
    {
        match self.get_mut().stream_type {
            StreamType::Tcp(_, ref mut writer) => {
                AsyncWrite::poll_write(Pin::new(writer), cx, buf)
            },
            StreamType::BufferedTcp(_, ref mut writer) => {
                AsyncWrite::poll_write(Pin::new(writer), cx, buf)
            },
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut().stream_type {
            StreamType::Tcp(_, ref mut writer) => {
                AsyncWrite::poll_flush(Pin::new(writer), cx)
            },
            StreamType::BufferedTcp(_, ref mut writer) => {
                AsyncWrite::poll_flush(Pin::new(writer), cx)
            },
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut().stream_type {
            StreamType::Tcp(_, ref mut writer) => {
                AsyncWrite::poll_shutdown(Pin::new(writer), cx)
            },
            StreamType::BufferedTcp(_, ref mut writer) => {
                AsyncWrite::poll_shutdown(Pin::new(writer), cx)
            },
        }
    }
}
