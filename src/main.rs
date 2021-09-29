//use std::path::{Path, PathBuf};

use std::{
    fs::File,
    io::BufWriter,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use argh::FromArgs;
use assembly_pack::sd0::stream::SegmentedStream;
use assembly_xml::universe_config::{CdnInfo, Environment};
use color_eyre::eyre::Context;
use futures_util::TryStreamExt;
use log::info;
use manifest::{load_manifest, FileLine};
use reqwest::Url;
use terminal_menu::{button, label, menu, mut_menu, run};
use tokio::io::{AsyncBufRead, BufReader};
use tokio_util::io::StreamReader;

use crate::{config::PatcherConfig, util::into_io_error};

mod config;
mod manifest;
mod util;

fn live() -> String {
    String::from("live")
}

#[derive(FromArgs)]
/// Run the LU patcher
struct Options {
    /// the base URL of the patch server
    #[argh(option)]
    cfg_url: String,

    /// the base URL of the patch server
    #[argh(option, default = "live()")]
    env: String,

    /// the installation directory
    #[argh(option)]
    install_dir: Option<PathBuf>,
}

fn join(base: &mut PathBuf, dir: &Path) {
    for c in dir.components() {
        match c {
            Component::Prefix(_) => todo!(),
            Component::RootDir => {
                *base = dir.to_owned();
                break;
            }
            Component::CurDir => { /* ignore */ }
            Component::ParentDir => {
                base.pop();
            }
            Component::Normal(v) => {
                base.push(v);
            }
        }
    }
}

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

struct Patcher {
    url: Url,
    config: PatcherConfig,
}

impl Patcher {
    fn get_url(&self, f: &FileLine) -> color_eyre::Result<Url> {
        let suffix = f.to_path();
        let url = self.url.join(&suffix)?;
        Ok(url)
    }

    fn config_key(&self) -> String {
        format!("{}/patcher.ini", self.config.patcherdirectory)
    }

    fn install_file_key(&self) -> String {
        format!(
            "{}/{}",
            self.config.installerdirectory, self.config.installfile
        )
    }
}

struct Downloader {
    client: reqwest::Client,
}

impl Downloader {
    fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    async fn setup_patcher(&self, cdn_info: &CdnInfo) -> color_eyre::Result<Patcher> {
        // Find the patcher URL
        let p_base = if cdn_info.secure {
            format!("https://{}/", &cdn_info.patcher_url)
        } else {
            format!("http://{}/", &cdn_info.patcher_url)
        };
        let p_host = Url::parse(&p_base)?;
        let p_dir_segment = format!("{}/", cdn_info.patcher_dir);
        let url = p_host.join(&p_dir_segment)?;
        let config_url = url.join("patcher.ini")?;

        info!("Config: {}", config_url);

        let config_str = self.get_text(config_url).await?;
        let config = PatcherConfig::from_str(&config_str)?;

        info!("Downloaded patcher config");

        Ok(Patcher { url, config })
    }

    async fn download(
        &mut self,
        url: Url,
        download_dir: &Path,
        output_path: &Path,
    ) -> color_eyre::Result<()> {
        let mut sd0_filename = output_path.file_name().unwrap().to_owned();
        sd0_filename.push(".sd0");
        let sd0_path = download_dir.join(sd0_filename);
        info!("saving to {}", sd0_path.display());

        // Stream the compressed file to disk
        let mut byte_stream = self.get_bytes_tokio(url).await?;
        stream_to_file(&sd0_path, &mut byte_stream).await?;

        info!(
            "download index complete, decompressing to {}",
            output_path.display()
        );

        // Create the parent folder
        let output_dir = output_path.parent().unwrap();
        tokio::fs::create_dir_all(output_dir).await?;

        // Decompress the file
        decompress_sd0(&sd0_path, output_path)?;

        info!("removing compressed index file");
        std::fs::remove_file(&sd0_path)?;
        Ok(())
    }

    async fn get(&self, url: Url) -> color_eyre::Result<reqwest::Response> {
        let text = self.client.get(url).send().await?;
        Ok(text)
    }

    async fn get_text(&self, url: Url) -> color_eyre::Result<String> {
        let text = self.get(url).await?.text().await?;
        Ok(text)
    }

    async fn get_bytes_tokio(&self, url: Url) -> color_eyre::Result<impl tokio::io::AsyncBufRead> {
        let stream = self.get(url).await?.bytes_stream().map_err(into_io_error);
        let reader = StreamReader::new(stream);
        Ok(reader)
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> color_eyre::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_module("lux_patcher", log::LevelFilter::Info)
        .init();
    let args: Options = argh::from_env();

    // Create client
    let mut client = Downloader::new();

    // Cleanup base parameter
    let options = Url::options();
    let api = Url::parse(&args.cfg_url)?;
    let base_url = options.base_url(Some(&api));

    // Get universe config
    let mut env_info_url = base_url.parse("UniverseConfig.svc/xml/EnvironmentInfo")?;

    info!("Environment: {}", &args.env);
    let env_query = format!("environment={}", &args.env);
    env_info_url.set_query(Some(&env_query));

    info!("Loading {}", env_info_url);

    // Get the environment info
    let env_info_xml = client.get_text(env_info_url).await?;
    let env_info: Environment = assembly_xml::quick::de::from_str(&env_info_xml)?;

    info!("Found {} universe(s)", env_info.servers.servers.len());

    // Present the universe selection menu
    let label_iter = Some(label("Select a universe:")).into_iter();
    let button_iter = env_info.servers.servers.iter().map(|s| &s.name).map(button);
    let buttons = label_iter.chain(button_iter).collect();
    let menu = menu(buttons);

    run(&menu);

    // you can get the selected buttons name like so:
    let sel = mut_menu(&menu).selected_item_index() - 1;
    let server = &env_info.servers.servers[sel];

    info!("Selected: {}", server.name);
    info!("{:?}", server.cdn_info);

    let patcher = client.setup_patcher(&server.cdn_info).await?;

    let install_dir = {
        let mut dir = std::env::current_dir()?;
        let install_path = args
            .install_dir
            .as_deref()
            .unwrap_or_else(|| Path::new(&patcher.config.defaultinstallpath));
        join(&mut dir, install_path);
        dir
    };

    info!("Install dir: {}", install_dir.display());
    std::fs::create_dir_all(&install_dir)?;

    let download_dir = install_dir.join(&patcher.config.downloaddirectory);
    info!("Download dir: {}", download_dir.display());
    std::fs::create_dir_all(&download_dir)?;

    let version_url = patcher.url.join(&patcher.config.versionfile)?;
    info!("Version file: {}", version_url);

    // Get trunk.txt
    let byte_stream = client.get_bytes_tokio(version_url).await?;

    let versions = load_manifest(byte_stream).await?;
    info!(
        "Loading manifest {} (version {})",
        versions.version.name, versions.version.version
    );
    info!("Found {} file(s)!", versions.files.len());

    let patcher_config_key = patcher.config_key();
    if let Some(f) = versions.files.get(&patcher_config_key) {
        let patcher_config_url = patcher.get_url(f)?;
        info!("patcher config is {}", patcher_config_url);

        let patcher_config_path = install_dir.join(patcher_config_key);
        client
            .download(patcher_config_url, &download_dir, &patcher_config_path)
            .await?;
    }

    let install_file_key = patcher.install_file_key();
    if let Some(f) = versions.files.get(&install_file_key) {
        info!("installer is {:?} (ignoring)", &f.hash);
    }

    if let Some(f) = versions.files.get(&patcher.config.indexfile) {
        let index_url = patcher.get_url(f)?;
        info!("index is {}", &index_url);

        let index_path = download_dir.join(&patcher.config.indexfile);
        client
            .download(index_url, &download_dir, &index_path)
            .await?;

        let index_file = tokio::fs::File::open(&index_path).await?;
        let index_reader = BufReader::new(index_file);
        let index = load_manifest(index_reader).await?;

        info!(
            "Loading manifest {} (version {})",
            index.version.name, index.version.version
        );
        info!("Found {} file(s)!", index.files.len());
    }

    Ok(())
}
