use log::info;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;

use assembly_pack::txt::{self, Manifest};

pub(crate) async fn load_manifest<B>(stream: B) -> color_eyre::Result<Manifest>
where
    B: tokio::io::AsyncBufRead + Unpin,
{
    let mut lines = LinesStream::new(stream.lines());
    let m = txt::load_manifest(&mut lines).await?;

    info!(
        "Loading manifest {} (version {})",
        &m.version.name, &m.version.version
    );
    info!("Found {} file(s)!", &m.files.len());

    Ok(m)
}
