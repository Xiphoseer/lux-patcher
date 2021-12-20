//! Data for `boot.cfg`

use regex::Regex;
use serde::Serialize;
use std::{borrow::Cow, fmt};

struct LDFWriter<T> {
    inner: T,
    first: bool,
    delim: String,
}

impl<T: std::fmt::Write> LDFWriter<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            first: true,
            delim: ",".to_string(),
        }
    }

    fn check(&mut self) -> Result<(), fmt::Error> {
        if self.first {
            self.first = false;
            Ok(())
        } else {
            self.inner.write_str(&self.delim)
        }
    }

    pub fn set_delim(&mut self, delim: String) {
        self.delim = delim;
    }

    pub fn write_str(&mut self, key: &str, value: &str) -> Result<(), fmt::Error> {
        self.check()?;
        write!(self.inner, "{}=0:{}", key, value)
    }

    pub fn write_i32(&mut self, key: &str, value: i32) -> Result<(), fmt::Error> {
        self.check()?;
        write!(self.inner, "{}=1:{}", key, value)
    }

    pub fn write_u32(&mut self, key: &str, value: u32) -> Result<(), fmt::Error> {
        self.check()?;
        write!(self.inner, "{}=5:{}", key, value)
    }

    pub fn write_bool(&mut self, key: &str, value: bool) -> Result<(), fmt::Error> {
        self.check()?;
        if value {
            write!(self.inner, "{}=7:1", key)
        } else {
            write!(self.inner, "{}=7:0", key)
        }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl BootConfig {
    pub fn to_cfg(&self) -> Result<String, fmt::Error> {
        let mut writer = LDFWriter::new(String::new());
        writer.set_delim(",\r\n".to_string());
        writer.write_str("SERVERNAME", &self.server_name)?;
        writer.write_str("PATCHSERVERIP", &self.patch_server_ip)?;
        writer.write_i32("PATCHSERVERPORT", self.patch_server_port)?;
        writer.write_str("AUTHSERVERIP", &self.auth_server_ip)?;
        writer.write_i32("LOGGING", self.logging)?;
        writer.write_u32("DATACENTERID", self.data_center_id)?;
        writer.write_i32("CPCODE", self.cp_code)?;
        writer.write_bool("AKAMAIDLM", self.akamai_dlm)?;
        writer.write_str("AKAMAIDLM", &self.patch_server_dir)?;
        writer.write_bool("UGCUSE3DSERVICES", self.ugc_use_3d_services)?;
        writer.write_str("UGCSERVERIP", &self.ugc_server_ip)?;
        writer.write_str("UGCSERVERDIR", &self.ugc_server_dir)?;
        writer.write_str("MANIFESTFILE", &self.manifest_file)?;
        writer.write_str("PASSURL", &self.passurl)?;
        writer.write_str("SIGNINURL", &self.sign_in_url)?;
        writer.write_str("SIGNUPURL", &self.sign_up_url)?;
        writer.write_str("REGISTERURL", &self.register_url)?;
        writer.write_str("CRASHLOGURL", &self.crash_log_url)?;
        writer.write_str("LOCALE", &self.locale)?;
        writer.write_bool("TRACK_DSK_USAGE", self.track_disk_usage)?;
        Ok(writer.into_inner())
    }
}

#[derive(Serialize)]
pub struct BootConfig {
    #[serde(rename = "SERVERNAME")]
    pub server_name: String,
    #[serde(rename = "PATCHSERVERIP")]
    pub patch_server_ip: String,
    #[serde(rename = "PATCHSERVERPORT")]
    pub patch_server_port: i32,
    #[serde(rename = "AUTHSERVERIP")]
    pub auth_server_ip: String,
    #[serde(rename = "LOGGING")]
    pub logging: i32,
    #[serde(rename = "DATACENTERID")]
    pub data_center_id: u32,
    #[serde(rename = "CPCODE")]
    pub cp_code: i32,
    #[serde(rename = "AKAMAIDLM")]
    pub akamai_dlm: bool,
    #[serde(rename = "AKAMAIDLM")]
    pub patch_server_dir: String,
    #[serde(rename = "UGCUSE3DSERVICES")]
    pub ugc_use_3d_services: bool,
    #[serde(rename = "UGCSERVERIP")]
    pub ugc_server_ip: String,
    #[serde(rename = "UGCSERVERDIR")]
    pub ugc_server_dir: String,
    #[serde(rename = "MANIFESTFILE")]
    pub manifest_file: String,
    #[serde(rename = "PASSURL")]
    pub passurl: String,
    #[serde(rename = "SIGNINURL")]
    pub sign_in_url: String,
    #[serde(rename = "SIGNUPURL")]
    pub sign_up_url: String,
    #[serde(rename = "REGISTERURL")]
    pub register_url: String,
    #[serde(rename = "CRASHLOGURL")]
    pub crash_log_url: String,
    #[serde(rename = "LOCALE")]
    pub locale: String,
    #[serde(rename = "TRACK_DSK_USAGE")]
    pub track_disk_usage: bool,
}

pub struct Token<'a> {
    pub install_path: std::borrow::Cow<'a, str>,
}

impl<'a> Token<'a> {
    pub fn resolve(self, input: &str) -> Cow<str> {
        let pattern = Regex::new(r"\{%([a-z]+)\}").unwrap();
        pattern.replace(input, self)
    }
}

impl<'a> regex::Replacer for Token<'a> {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        let m = caps.get(1).unwrap();
        if m.as_str() == "installpath" {
            dst.push_str(&self.install_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    #[test]
    fn test_token() {
        let token = super::Token {
            install_path: Cow::Borrowed("some\\path"),
        };
        let res = token.resolve("{%installpath}\\client\\boot.cfg");
        assert_eq!(res.as_ref(), "some\\path\\client\\boot.cfg")
    }
}
