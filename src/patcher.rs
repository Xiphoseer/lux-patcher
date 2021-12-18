use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use assembly_pack::{
    pki::core::PackIndexFile,
    txt::{FileLine, Manifest},
};
use assembly_xml::universe_config::CdnInfo;
use color_eyre::eyre::{eyre, WrapErr};
use log::info;
use reqwest::Url;
use tokio::io::BufReader;

use crate::{
    cache::{Cache, CacheEntry, CacheKey},
    config::PatcherConfig,
    crc::calculate_crc,
    download::Downloader,
    manifest::load_manifest,
    util::join,
};

pub struct PatcherBuilder {
    pub url: Url,
    pub config: PatcherConfig,
}

impl PatcherBuilder {
    pub async fn setup(net: &Downloader, cdn_info: &CdnInfo) -> color_eyre::Result<Self> {
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

        let config_str = net.get_text(config_url).await?;
        let config = config_str.parse()?;

        info!("Downloaded patcher config");

        Ok(PatcherBuilder { url, config })
    }

    pub fn build(self, net: Downloader, install_dir: Option<&Path>) -> color_eyre::Result<Patcher> {
        let dirs = PatcherDirs::new(&self.config, install_dir)?;
        let keys = PatcherKeys {
            download: format!("{}/", &self.config.downloaddirectory),
            install: String::new(),
        };
        Ok(Patcher {
            url: self.url,
            config: self.config,
            net,
            dirs,
            keys,
        })
    }
}

pub struct PatcherDirs {
    pub install: PathBuf,
    pub download: PathBuf,
}

impl PatcherDirs {
    pub fn new(cfg: &PatcherConfig, install_dir: Option<&Path>) -> std::io::Result<Self> {
        let install = {
            let mut dir = std::env::current_dir()?;
            let install_path = install_dir.unwrap_or_else(|| Path::new(&cfg.defaultinstallpath));
            join(&mut dir, install_path);
            dir
        };
        info!("Install dir: {}", install.display());
        std::fs::create_dir_all(&install)?;

        let download = install.join(&cfg.downloaddirectory);
        info!("Download dir: {}", download.display());
        std::fs::create_dir_all(&download)?;

        Ok(Self { install, download })
    }
}

pub struct PatcherKeys {
    download: String,
    install: String,
}

pub struct Patcher {
    pub url: Url,
    pub config: PatcherConfig,
    pub net: Downloader,
    pub dirs: PatcherDirs,
    pub keys: PatcherKeys,
}

impl Patcher {
    pub async fn load_manifest(&self, manifestfile: &str) -> color_eyre::Result<Manifest> {
        let path = self.dirs.download.join(manifestfile);
        let file = tokio::fs::File::open(&path).await?;
        let reader = BufReader::new(file);
        let manifest = load_manifest(reader).await?;
        Ok(manifest)
    }

    pub async fn ensure_meta(
        &self,
        cache: &mut Cache,
        manifest: &Manifest,
        file: &str,
    ) -> color_eyre::Result<bool> {
        self.ensure(
            cache,
            manifest,
            &self.dirs.download,
            &self.keys.download,
            file,
        )
        .await
        .wrap_err_with(|| eyre!("Failed to ensure meta {}", file))
    }

    pub async fn ensure_file(
        &self,
        cache: &mut Cache,
        pki: &PackIndexFile,
        manifest: &Manifest,
        file: &str,
    ) -> color_eyre::Result<bool> {
        let crc = calculate_crc(file.as_bytes());
        if let Some(meta) = pki.files.get(&crc) {
            info!(
                "{} is cat {} in {}",
                file, meta.category, &pki.archives[meta.pack_file as usize].path
            );
            Ok(false)
        } else {
            self.ensure(
                cache,
                manifest,
                &self.dirs.install,
                &self.keys.install,
                file,
            )
            .await
            .wrap_err_with(|| eyre!("Failed to ensure {}", file))
        }
    }

    async fn ensure(
        &self,
        cache: &mut Cache,
        manifest: &Manifest,
        base_dir: &Path,
        base_key: &str,
        file: &str,
    ) -> color_eyre::Result<bool> {
        let path = base_dir.join(file);
        if let Some(f) = manifest.files.get(file) {
            let url = self.get_url(f)?;
            info!("{} is {}", file, &url);

            // Get cache key
            let cache_key = base_key.to_owned() + file;
            let cache_key = CacheKey::new(&cache_key);

            // Check whether the file needs to be downloaded
            let mut needs_download = true;
            if let Some(c) = cache.get(&cache_key) {
                if c.hash == f.hash {
                    needs_download = false;
                }
            }

            // Download the file
            if needs_download {
                self.net.download(url, &self.dirs.download, &path).await?;
                let meta = tokio::fs::metadata(&path).await?;

                let mtime = {
                    let time = meta.modified()?;
                    let dur = time.duration_since(SystemTime::UNIX_EPOCH)?;
                    dur.as_secs_f64()
                };
                cache.insert(
                    cache_key,
                    CacheEntry {
                        mtime,
                        size: f.filesize,
                        hash: f.hash,
                    },
                );
            }
            Ok(true)
        } else {
            log::warn!("{} not found in manifest!", file);
            Ok(false)
        }
    }

    pub fn get_url(&self, f: &FileLine) -> color_eyre::Result<Url> {
        let suffix = f.to_path();
        let url = self.url.join(&suffix)?;
        Ok(url)
    }

    pub fn config_key(&self) -> String {
        format!("{}/patcher.ini", self.config.patcherdirectory)
    }

    pub fn install_file_key(&self) -> String {
        format!(
            "{}/{}",
            self.config.installerdirectory, self.config.installfile
        )
    }
}
