mod lines;

use std::collections::BTreeMap;

use color_eyre::eyre::eyre;
use log::info;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, Lines};

use crate::manifest::lines::version_line;

use self::lines::file_line;
pub(crate) use self::lines::{FileLine, VersionLine, MD5};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Section {
    Version,
    Files,
}

impl Section {
    fn as_header(&self) -> &'static str {
        match self {
            Self::Files => "[files]",
            Self::Version => "[version]",
        }
    }
}

async fn expect_header<R>(lines: &mut Lines<R>, section: Section) -> color_eyre::Result<()>
where
    R: AsyncBufRead + Unpin,
{
    let header = section.as_header();
    let line = lines
        .next_line()
        .await?
        .ok_or_else(|| eyre!("Missing '{}' header", header))?;
    if line != header {
        return Err(eyre!("Expected '{}' header, got {:?}", header, line));
    }

    Ok(())
}

async fn read_index_version<R>(lines: &mut Lines<R>) -> color_eyre::Result<VersionLine>
where
    R: AsyncBufRead + Unpin,
{
    expect_header(lines, Section::Version).await?;
    let line = lines
        .next_line()
        .await?
        .ok_or_else(|| eyre!("Missing version line"))?;
    let version = version_line(&line)?;
    Ok(version)
}

pub struct Manifest {
    pub version: VersionLine,
    pub files: BTreeMap<String, FileLine>,
}

pub(crate) async fn load_manifest<B>(stream: B) -> color_eyre::Result<Manifest>
where
    B: tokio::io::AsyncBufRead + Unpin,
{
    let r = tokio::io::BufReader::new(stream);

    let mut lines = r.lines();
    let mut files = BTreeMap::new();

    let version = read_index_version(&mut lines).await?;
    expect_header(&mut lines, Section::Files).await?;
    while let Some(item) = lines.next_line().await? {
        let line = item;
        let (filename, data) = file_line(&line)?;
        files.insert(filename.to_owned(), data);
    }

    info!(
        "Loading manifest {} (version {})",
        &version.name, &version.version
    );
    info!("Found {} file(s)!", &files.len());

    Ok(Manifest { version, files })
}
