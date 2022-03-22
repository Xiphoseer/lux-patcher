use std::{convert::Infallible, fmt::Display, path::PathBuf, str::FromStr};

#[derive(Debug, Default, Clone)]
pub struct ExcludeList {
    #[allow(dead_code)]
    paths: Vec<PathBuf>,
}

impl FromStr for ExcludeList {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let paths = s.split(',').map(PathBuf::from).collect();
        Ok(Self { paths })
    }
}

#[derive(Debug, Clone)]
pub struct PatcherConfig {
    pub patcherexeversion: String,
    pub serverdirectory: String,
    pub downloaddirectory: String,
    pub patcherdirectory: String,
    pub installerdirectory: String,
    pub versionfile: String,
    pub indexfile: String,
    pub defaultmanifestfile: String,
    pub minimalmanifestfile: String,
    pub hotfixmanifestfile: String,
    pub packcatalog: String,
    pub defaultinstallpath: String,
    pub installkey: String,
    pub installfile: String,
    pub configfile: String,
    pub win_exclude: ExcludeList,
    pub mac_exclude: ExcludeList,
    pub noclean: Vec<PathBuf>,
    pub caption: String,
    pub cachefile: String,
    pub check: bool,
    pub quickcheck: bool,
    pub clean: bool,
    pub log: bool,
    pub waitstart: bool,
    pub usedefaultinstallpath: bool,
    pub usedynamicdownload: bool,
}

impl std::default::Default for PatcherConfig {
    fn default() -> Self {
        Self {
            patcherexeversion: Default::default(),
            serverdirectory: "lwoclient".to_string(),
            downloaddirectory: "versions".to_string(),
            patcherdirectory: "patcher".to_string(),
            installerdirectory: "installer".to_string(),
            versionfile: "version.txt".to_string(),
            indexfile: "index.txt".to_string(),
            defaultmanifestfile: "trunk.txt".to_string(),
            minimalmanifestfile: "frontend.txt".to_string(),
            hotfixmanifestfile: "hotfix.txt".to_string(),
            packcatalog: "primary.pki".to_string(),
            defaultinstallpath: "..".to_string(),
            installkey: "Software\\NetDevil\\LEGO Universe".to_string(),
            installfile: "lego_universe_install.exe".to_string(),
            configfile: "{%installpath}\\client\\boot.cfg".to_string(),
            win_exclude: ExcludeList {
                paths: vec![
                    PathBuf::from("client/legouniverse_mac.exe"),
                    PathBuf::from("client/stlport.5.2.dll"),
                    PathBuf::from("cider/*"),
                    PathBuf::from("patcher/*"),
                ],
            },
            mac_exclude: ExcludeList {
                paths: vec![
                    PathBuf::from("client/legouniverse.exe"),
                    PathBuf::from("client/d3dx9_34.dll"),
                    PathBuf::from("client/awesomium.dll"),
                    PathBuf::from("patcher/*"),
                ],
            },
            noclean: vec![],
            caption: "LEGO Universe Updater".to_string(),
            cachefile: "quickcheck.txt".to_string(),
            check: true,
            quickcheck: true,
            clean: true,
            log: true,
            waitstart: true,
            usedefaultinstallpath: true,
            usedynamicdownload: true,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    UnknownKey(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKey(k) => write!(f, "Unknown key '{}'", k),
        }
    }
}

impl std::error::Error for Error {}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

fn is_true(s: &str) -> bool {
    matches!(s, "Yes" | "True")
}

impl FromStr for PatcherConfig {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut cfg = Self::default();
        for line in s.split('\n') {
            if line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim();
                let value = value.trim_matches('"');
                match key {
                    "patcherexeversion" => cfg.patcherexeversion = value.parse()?,
                    "serverdirectory" => cfg.serverdirectory = value.parse()?,
                    "downloaddirectory" => cfg.downloaddirectory = value.parse()?,
                    "patcherdirectory" => cfg.patcherdirectory = value.parse()?,
                    "installerdirectory" => cfg.installerdirectory = value.parse()?,
                    "versionfile" => cfg.versionfile = value.parse()?,
                    "indexfile" => cfg.indexfile = value.parse()?,
                    "defaultmanifestfile" => cfg.defaultmanifestfile = value.parse()?,
                    "minimalmanifestfile" => cfg.minimalmanifestfile = value.parse()?,
                    "hotfixmanifestfile" => cfg.hotfixmanifestfile = value.parse()?,
                    "packcatalog" => cfg.packcatalog = value.parse()?,
                    "defaultinstallpath" => cfg.defaultinstallpath = value.parse()?,
                    "installkey" => cfg.installkey = value.parse()?,
                    "installfile" => cfg.installfile = value.parse()?,
                    "configfile" => cfg.configfile = value.parse()?,
                    "win_exclude" => cfg.win_exclude = value.parse()?,
                    "mac_exclude" => cfg.mac_exclude = value.parse()?,
                    "noclean" => cfg.noclean.push(PathBuf::from(value)),
                    "caption" => cfg.caption = value.parse()?,
                    "cachefile" => cfg.cachefile = value.parse()?,
                    "check" => cfg.check = is_true(value),
                    "quickcheck" => cfg.quickcheck = is_true(value),
                    "clean" => cfg.clean = is_true(value),
                    "log" => cfg.log = is_true(value),
                    "waitstart" => cfg.waitstart = is_true(value),
                    "usedefaultinstallpath" => cfg.usedefaultinstallpath = is_true(value),
                    "usedynamicdownload" => cfg.usedynamicdownload = is_true(value),
                    _ => return Err(Error::UnknownKey(key.to_owned())),
                }
            }
        }
        Ok(cfg)
    }
}
