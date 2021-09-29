use std::{fs::File, io::BufWriter, path::Path};

use assembly_pack::sd0::stream::SegmentedStream;
use color_eyre::eyre::Context;
use futures_util::TryStreamExt;
use log::info;
use reqwest::Url;
use tokio::io::AsyncBufRead;
use tokio_util::io::StreamReader;

use crate::util::into_io_error;

async fn stream_to_file<S>(path: &Path, mut bytes: &mut S) -> color_eyre::Result<()>
where
    S: AsyncBufRead + Unpin,
{
    let mut file = tokio::fs::File::create(path).await?;
    tokio::io::copy(&mut bytes, &mut file).await?;
    Ok(())
}

fn decompress_sd0(input: &Path, output: &Path) -> color_eyre::Result<()> {
    let file = File::open(input)?;
    let mut buf = std::io::BufReader::new(file);
    let mut stream = SegmentedStream::new(&mut buf)?;

    let out = File::create(output)?;
    let mut writer = BufWriter::new(out);

    std::io::copy(&mut stream, &mut writer).context("Streaming sd0 file")?;
    Ok(())
}

pub struct Downloader {
    client: reqwest::Client,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn download(
        &self,
        url: Url,
        download_dir: &Path,
        path: &Path,
    ) -> color_eyre::Result<()> {
        let mut sd0_filename = path.file_name().unwrap().to_owned();
        sd0_filename.push(".sd0");
        let sd0_path = download_dir.join(sd0_filename);
        info!("saving to {}", sd0_path.display());

        // Stream the compressed file to disk
        let mut byte_stream = self.get_bytes_tokio(url).await?;
        stream_to_file(&sd0_path, &mut byte_stream).await?;

        info!("download complete, decompressing to {}", path.display());

        // Create the parent folder
        let output_dir = path.parent().unwrap();
        tokio::fs::create_dir_all(output_dir).await?;

        // Decompress the file
        decompress_sd0(&sd0_path, path)?;

        info!("removing compressed file");
        std::fs::remove_file(&sd0_path)?;
        Ok(())
    }

    pub async fn get(&self, url: Url) -> color_eyre::Result<reqwest::Response> {
        let text = self.client.get(url).send().await?;
        Ok(text)
    }

    pub async fn get_text(&self, url: Url) -> color_eyre::Result<String> {
        let text = self.get(url).await?.text().await?;
        Ok(text)
    }

    pub async fn get_bytes_tokio(
        &self,
        url: Url,
    ) -> color_eyre::Result<impl tokio::io::AsyncBufRead> {
        let stream = self.get(url).await?.bytes_stream().map_err(into_io_error);
        let reader = StreamReader::new(stream);
        Ok(reader)
    }
}
