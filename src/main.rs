//use std::path::{Path, PathBuf};

use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use argh::FromArgs;
use assembly_pack::sd0::stream::SegmentedStream;
use assembly_xml::universe_config::Environment;
use color_eyre::eyre::Context;
use futures_util::TryStreamExt;
use log::info;
use manifest::load_manifest;
use reqwest::Url;
use terminal_menu::{button, label, menu, mut_menu, run};
use tokio::io::AsyncBufRead;
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
    let mut buf = BufReader::new(file);
    let mut stream = SegmentedStream::new(&mut buf)?;

    let out = File::create(output)?;
    let mut writer = BufWriter::new(out);

    std::io::copy(&mut stream, &mut writer).context("Streaming sd0 file")?;
    Ok(())
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
    let client = Downloader::new();

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

    // Find the patcher URL
    let p_base = if server.cdn_info.secure {
        format!("https://{}/", &server.cdn_info.patcher_url)
    } else {
        format!("http://{}/", &server.cdn_info.patcher_url)
    };
    let p_host = Url::parse(&p_base)?;
    let p_dir_segment = format!("{}/", server.cdn_info.patcher_dir);
    let patcher_url = p_host.join(&p_dir_segment)?;
    let patcher_config_url = patcher_url.join("patcher.ini")?;

    info!("Config: {}", patcher_config_url);

    let patcher_config_str = client.get_text(patcher_config_url).await?;
    let patcher_config = PatcherConfig::from_str(&patcher_config_str)?;

    info!("Downloaded patcher config");

    let install_dir = {
        let mut dir = std::env::current_dir()?;
        let install_path = args
            .install_dir
            .as_deref()
            .unwrap_or_else(|| Path::new(&patcher_config.defaultinstallpath));
        join(&mut dir, install_path);
        dir
    };

    info!("Install dir: {}", install_dir.display());
    std::fs::create_dir_all(&install_dir)?;

    let download_dir = install_dir.join(patcher_config.downloaddirectory);
    info!("Download dir: {}", download_dir.display());
    std::fs::create_dir_all(&download_dir)?;

    let version_url = patcher_url.join(&patcher_config.versionfile)?;
    info!("Version file: {}", version_url);

    // Get trunk.txt
    let byte_stream = client.get_bytes_tokio(version_url).await?;

    let versions = load_manifest(byte_stream).await?;
    info!(
        "Loading manifest {} (version {})",
        versions.version.name, versions.version.version
    );
    info!("Found {} file(s)!", versions.files.len());

    let patcher_config_key = format!("{}/patcher.ini", patcher_config.patcherdirectory);
    let install_file_key = format!(
        "{}/{}",
        patcher_config.installerdirectory, patcher_config.installfile
    );

    if let Some(f) = versions.files.get(&patcher_config_key) {
        info!("patcher config is {:?} (ignoring)", &f.hash);
    }

    if let Some(f) = versions.files.get(&install_file_key) {
        info!("installer is {:?} (ignoring)", &f.hash);
    }

    if let Some(f) = versions.files.get(&patcher_config.indexfile) {
        let index_suffix = f.to_path();
        let index_url = patcher_url.join(&index_suffix)?;
        let index_sd0_filename = format!("{}.sd0", patcher_config.indexfile);
        let index_sd0_path = download_dir.join(index_sd0_filename);
        let index_path = download_dir.join(&patcher_config.indexfile);
        info!("index is {}", &index_url);
        info!("saving index to {}", index_sd0_path.display());

        let mut byte_stream = client.get_bytes_tokio(index_url).await?;
        stream_to_file(&index_sd0_path, &mut byte_stream).await?;

        info!(
            "download index complete, decompressing to {}",
            index_path.display()
        );
        decompress_sd0(&index_sd0_path, &index_path)?;

        info!("removing compressed index file");
        std::fs::remove_file(&index_sd0_path)?;
    }

    Ok(())
}
