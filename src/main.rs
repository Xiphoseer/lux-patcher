use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
};

use argh::FromArgs;
use assembly_pack::pki::core::PackIndexFile;
use assembly_xml::universe_config::Environment;
use color_eyre::eyre::{eyre, Context};
use log::{info, warn};
use manifest::load_manifest;
use reqwest::Url;
use terminal_menu::{button, label, menu, mut_menu, run};

use crate::{cache::Cache, download::Downloader, patcher::PatcherBuilder};

mod boot;
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
    } else {
        warn!("patcher config {:?} not found", patcher_config_key);
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

    // Load the manifests
    // Need to download both files, so that the default/trunk manifest is there for the on-demand variant
    patcher
        .ensure_meta(&mut cache, &index, &patcher.config.defaultmanifestfile)
        .await?;
    patcher
        .ensure_meta(&mut cache, &index, &patcher.config.minimalmanifestfile)
        .await?;

    // Load the pack catalog
    let has_pki = patcher
        .ensure_meta(&mut cache, &index, &patcher.config.packcatalog)
        .await?;

    let pki = if has_pki {
        let catalog_file = patcher.dirs.download.join(&patcher.config.packcatalog);
        let file = std::fs::File::open(&catalog_file)
            .wrap_err_with(|| eyre!("Failed to open {}", catalog_file.display()))?;
        PackIndexFile::try_from(file).map_err(|e| eyre!("Failed to load PKI file: {:?}", e))?
    } else {
        // PKI file with nothing
        log::info!("Assuming empty PK catalog");
        PackIndexFile {
            archives: vec![],
            files: Default::default(),
        }
    };

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

    let manifest = patcher.load_manifest(manifestfile).await?;

    for key in manifest.files.keys() {
        patcher
            .ensure_file(&mut cache, &pki, &manifest, key)
            .await?;
    }

    cache.save(&cache_path)?;

    // Create boot.cfg

    let token = boot::Token {
        install_path: patcher.dirs.install.to_string_lossy(),
    };
    let canonical_config_file = patcher.config.configfile.replace('\\', "/");
    let configfile = token.resolve(&canonical_config_file);

    info!("Config file: {:?}", configfile);
    let patch_server_port = if server.cdn_info.secure { 443 } else { 80 };
    let config = boot::BootConfig {
        server_name: server.name.clone(),
        patch_server_ip: server.cdn_info.patcher_url.clone(),
        patch_server_port,
        auth_server_ip: server.authentication_ip.clone(),
        logging: server.log_level as i32,
        data_center_id: server.data_center_id,
        cp_code: server.cdn_info.cp_code as i32,
        akamai_dlm: server.cdn_info.use_dlm,
        patch_server_dir: server.cdn_info.patcher_dir.clone(),
        ugc_use_3d_services: server.use3d_services,
        ugc_server_ip: server.ugc_cdn_info.patcher_url.clone(),
        ugc_server_dir: server.ugc_cdn_info.patcher_dir.clone(),
        manifest_file: patcher.config.defaultmanifestfile,
        passurl: env_info.account_info.send_password_url,
        sign_in_url: env_info.account_info.sign_in_url,
        sign_up_url: env_info.account_info.sign_up_url,
        register_url: env_info.game_info.client_url,
        crash_log_url: env_info.game_info.crash_log_url,
        locale: server.language.clone(),
        track_disk_usage: true,
        use_catalog: !pki.archives.is_empty(),
    };
    let config_path = Path::new(configfile.as_ref());
    let config_text = config.to_cfg()?;

    tokio::fs::write(config_path, config_text)
        .await
        .wrap_err_with(|| eyre!("Failed to write {}", config_path.display()))?;

    Ok(())
}
