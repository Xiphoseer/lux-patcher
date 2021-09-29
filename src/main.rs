use std::{convert::TryFrom, path::PathBuf};

use argh::FromArgs;
use assembly_pack::pki::core::PackIndexFile;
use assembly_xml::universe_config::Environment;
use color_eyre::eyre::eyre;
use log::info;
use manifest::load_manifest;
use reqwest::Url;
use terminal_menu::{button, label, menu, mut_menu, run};

use crate::{cache::Cache, download::Downloader, patcher::PatcherBuilder};

mod cache;
mod config;
mod crc;
mod download;
mod manifest;
mod patcher;
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> color_eyre::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_module("lux_patcher", log::LevelFilter::Info)
        .init();
    let args: Options = argh::from_env();

    // Create client
    let net = Downloader::new();

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
    let env_info_xml = net.get_text(env_info_url).await?;
    let env_info: Environment = assembly_xml::quick::de::from_str(&env_info_xml)?;

    info!("Found {} universe(s)", env_info.servers.servers.len());

    // Present the universe selection menu
    let label_iter = Some(label("Select a universe:")).into_iter();
    let button_iter = env_info.servers.servers.iter().map(|s| &s.name).map(button);
    let buttons = label_iter.chain(button_iter).collect();
    let universe_menu = menu(buttons);

    run(&universe_menu);

    // you can get the selected buttons name like so:
    let sel = mut_menu(&universe_menu).selected_item_index() - 1; // -1 for the label
    let server = &env_info.servers.servers[sel];

    info!("Selected: {}", server.name);
    info!("{:?}", server.cdn_info);

    let patcher_builder = PatcherBuilder::setup(&net, &server.cdn_info).await?;
    let install_dir = args.install_dir.as_deref();
    let patcher = patcher_builder.build(net, install_dir)?;

    let version_url = patcher.url.join(&patcher.config.versionfile)?;
    info!("Version file: {}", version_url);

    let byte_stream = patcher.net.get_bytes_tokio(version_url).await?;
    let versions = load_manifest(byte_stream).await?;

    let patcher_config_key = patcher.config_key();
    if let Some(f) = versions.files.get(&patcher_config_key) {
        let patcher_config_url = patcher.get_url(f)?;
        info!("patcher config is {}", patcher_config_url);

        let patcher_config_path = patcher.dirs.install.join(patcher_config_key);
        patcher
            .net
            .download(
                patcher_config_url,
                &patcher.dirs.download,
                &patcher_config_path,
            )
            .await?;
    }

    let install_file_key = patcher.install_file_key();
    if let Some(f) = versions.files.get(&install_file_key) {
        info!("installer is {:?} (ignoring)", &f.hash);
    }

    let mut cache = Cache::new();
    let cache_path = patcher.dirs.download.join(&patcher.config.cachefile);

    cache.load(&cache_path)?;

    // Ensure the index file is up to date
    patcher
        .ensure_meta(&mut cache, &versions, &patcher.config.indexfile)
        .await?;

    // Load the index file
    let index = patcher.load_manifest(&patcher.config.indexfile).await?;

    // Load the pack catalog
    patcher
        .ensure_meta(&mut cache, &index, &patcher.config.packcatalog)
        .await?;

    let catalog_file = patcher.dirs.download.join(&patcher.config.packcatalog);
    let file = std::fs::File::open(catalog_file)?;
    let pki =
        PackIndexFile::try_from(file).map_err(|e| eyre!("Failed to load PKI file: {:?}", e))?;

    let variant_menu = menu(vec![
        button(&format!("Minimal ({})", patcher.config.minimalmanifestfile)),
        button(&format!("Default ({})", patcher.config.defaultmanifestfile)),
    ]);

    run(&variant_menu);

    let sel = mut_menu(&variant_menu).selected_item_index();
    let manifestfile = if sel == 0 {
        &patcher.config.minimalmanifestfile
    } else {
        &patcher.config.defaultmanifestfile
    }
    .as_str();

    info!("Using manifest {}", manifestfile);

    patcher
        .ensure_meta(&mut cache, &index, manifestfile)
        .await?;

    let manifest = patcher.load_manifest(manifestfile).await?;

    for key in manifest.files.keys() {
        patcher
            .ensure_file(&mut cache, &pki, &manifest, key)
            .await?;
    }

    cache.save(&cache_path)?;

    Ok(())
}
