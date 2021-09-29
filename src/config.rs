use std::{convert::Infallible, fmt::Display, path::PathBuf, str::FromStr};

#[derive(Debug, Default, Clone)]
pub struct ExcludeList {
    paths: Vec<PathBuf>,
}

impl FromStr for ExcludeList {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let paths = s.split(',').map(PathBuf::from).collect();
        Ok(Self { paths })
    }
}

#[derive(Default, Debug, Clone)]
pub struct PatcherConfig {
    pub patcherexeversion: String,   //="14.00"
    pub serverdirectory: String,     //="lwoclient"
    pub downloaddirectory: String,   //="versions"
    pub patcherdirectory: String,    //="patcher"
    pub installerdirectory: String,  //="installer"
    pub versionfile: String,         //="version.txt"
    pub indexfile: String,           //="index.txt"
    pub defaultmanifestfile: String, //="trunk.txt"
    pub minimalmanifestfile: String, //="frontend.txt"
    pub hotfixmanifestfile: String,  //="hotfix.txt"
    pub packcatalog: String,         //="primary.pki"
    pub defaultinstallpath: String,  //=".."
    pub installkey: String,          //="Software\NetDevil\LEGO Universe"
    pub installfile: String,         //="lego_universe_install.exe"
    pub configfile: String,          //="{%installpath}\client\boot.cfg"
    pub win_exclude: ExcludeList, //=client/legouniverse_mac.exe,client/stlport.5.2.dll,cider/*,patcher/*
    pub mac_exclude: ExcludeList, //=client/legouniverse.exe,client/d3dx9_34.dll,client/awesomium.dll,patcher/*
    pub noclean: Vec<PathBuf>,
    pub caption: String,             //="LEGO Universe Updater"
    pub cachefile: String,           //="quickcheck.txt"
    pub check: bool,                 //="Yes"
    pub quickcheck: bool,            //="Yes"
    pub clean: bool,                 //="Yes"
    pub log: bool,                   //="Yes"
    pub waitstart: bool,             //="Yes"
    pub usedefaultinstallpath: bool, //="True"
    pub usedynamicdownload: bool,    //="True"
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
            if let Some((key, value)) = line.split_once("=") {
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
