use std::io;
use tokio::io::{AsyncRead, AsyncWrite, BufReader, BufWriter};
use tokio::net::TcpStream;
use std::pin::Pin;
use std::task::{Context, Poll};

// Based on merge-io crate, adapted to tokio::io::{AsyncRead, AsyncWrite} 

pub trait ReadWriteStream: AsyncRead + AsyncWrite + Unpin + Send {}

#[derive(Debug)]
pub struct MergeIO<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    reader: R,
    writer: W,
}

impl<R, W> MergeIO<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn buffered<Reader, Writer>(reader: Reader, writer: Writer)
        -> MergeIO<BufReader<Reader>, BufWriter<Writer>>
    where
        Reader: AsyncRead + Unpin,
        Writer: AsyncWrite + Unpin
    {
        MergeIO::new(
            BufReader::new(reader),
            BufWriter::new(writer)
        )
    }

    pub fn new(reader: R, writer: W) -> Self {
        MergeIO { reader, writer }
    }

    pub fn reader(&self) -> &R {
        &self.reader
    }

    pub fn writer(&self) -> &W {
        &self.writer
    }

    pub fn reader_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn into_inner(self) -> (R, W) {
        (self.reader, self.writer)
    }
}

impl<R, W> AsyncRead for MergeIO<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8])
        -> Poll<io::Result<usize>>
    {
        AsyncRead::poll_read(Pin::new(&mut self.get_mut().reader), cx, buf)
    }
}

impl<R, W> AsyncWrite for MergeIO<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.get_mut().writer), cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.get_mut().writer), cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.get_mut().writer), cx)
    }
}

impl<R, W> ReadWriteStream for MergeIO<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send
{
}
