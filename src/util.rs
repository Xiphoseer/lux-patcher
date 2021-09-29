use bytes::Bytes;
use futures_core::Stream;
use futures_util::{
    io::{AsyncBufRead, AsyncRead, BufReader, Lines},
    AsyncBufReadExt, TryStreamExt,
};

pub(crate) fn async_read_of<B>(stream: B) -> impl AsyncRead
where
    B: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    stream
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
        .into_async_read()
}

pub(crate) fn line_stream_of<B>(stream: B) -> Lines<impl AsyncBufRead>
where
    B: Stream<Item = reqwest::Result<Bytes>> + Unpin,
{
    let r = async_read_of(stream);
    BufReader::new(r).lines()
}
