use assembly_pack::md5::MD5Sum;
use std::{
    collections::BTreeMap,
    fmt::Display,
    fs::File,
    io::{BufRead, BufReader, BufWriter, ErrorKind, Write},
    path::Path,
    //    time::{SystemTime, SystemTimeError},
};

/*pub fn current_time_f64() -> Result<f64, SystemTimeError> {
    let time = SystemTime::now();
    let dur = time.duration_since(SystemTime::UNIX_EPOCH)?;
    Ok(dur.as_secs_f64())
}*/

/// One entry in the cache
pub struct CacheEntry {
    /// The time the file was written
    pub mtime: Option<f64>,
    /// The (uncompressed) size of the file
    pub size: u32,
    /// The (uncompressed) hash of the file
    pub hash: MD5Sum,
}

impl Display for CacheEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(mtime) = self.mtime {
            write!(f, "{:.6}", mtime)?;
        }
        write!(f, ",{},{:?}", self.size, self.hash)
    }
}

pub struct CacheKey(String);

impl CacheKey {
    pub fn new(v: &str) -> Self {
        Self(v.replace('/', "\\"))
    }
}

pub struct Cache {
    entries: BTreeMap<String, CacheEntry>,
}

impl Cache {
    /// Create a new cache
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Load from a cache file
    pub fn load(&mut self, path: &Path) -> std::io::Result<()> {
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let reader = BufReader::new(f);
        for line in reader.lines() {
            let line = line?;
            let mut parts = line.split(',');
            let key = CacheKey(parts.next().unwrap().to_owned());
            let mtime = parts.next().unwrap().parse().ok();
            let size = parts.next().unwrap().parse().unwrap();
            let hash = parts.next().unwrap().parse().unwrap();
            self.insert(key, CacheEntry { mtime, size, hash })
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let f = File::create(path)?;
        let mut writer = BufWriter::new(f);
        write!(writer, "{}", self)?;
        Ok(())
    }

    /// Check whether this key is present
    pub fn get(&self, key: &CacheKey) -> Option<&CacheEntry> {
        self.entries.get(&key.0)
    }

    /// Insert a new entry
    pub fn insert(&mut self, key: CacheKey, value: CacheEntry) {
        self.entries.insert(key.0, value);
    }
}

impl Display for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, value) in &self.entries {
            writeln!(f, "{},{}", key, value)?;
        }
        Ok(())
    }
}
