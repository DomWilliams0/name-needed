use common::*;
use parking_lot::{Mutex, MutexGuard};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

use crate::config::Config;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    Parsing(#[from] ron::de::Error),

    #[error("Failed to watch config file: {0}")]
    Notify(#[from] notify::Error),

    #[error("Path is not a file")]
    NotAFile,

    #[error("RawConfig path not initialized")]
    PathNotSet,
}

type ConfigResult<T> = std::result::Result<T, ConfigError>;

pub(crate) struct RawConfig {
    pub parsed: Option<Config>,
    pub path: Option<PathBuf>,
    pub load_time: Instant,

    watcher: Option<RecommendedWatcher>,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            load_time: Instant::now(),
            parsed: None,
            path: None,
            watcher: None,
        }
    }
}

impl RawConfig {
    pub fn parse_file(&mut self) -> ConfigResult<()> {
        let path = self.path.as_ref().ok_or(ConfigError::PathNotSet)?;
        let bytes = std::fs::read_to_string(path.as_path()).map_err(ConfigError::Io)?;
        self.parse_bytes(&bytes)
    }

    pub fn parse_bytes(&mut self, bytes: &str) -> ConfigResult<()> {
        let parsed = ron::de::from_str(bytes).map_err(ConfigError::Parsing)?;
        self.parsed = Some(parsed);
        self.load_time = Instant::now();

        Ok(())
    }
}

pub struct ConfigRef<'a> {
    config: MutexGuard<'a, RawConfig>,
}

impl<'a> Deref for ConfigRef<'a> {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        self.config.parsed.as_ref().unwrap()
    }
}

lazy_static! {
    static ref CONFIG: Mutex<RawConfig> = Mutex::new(RawConfig::default());
}

fn lock<'a>() -> MutexGuard<'a, RawConfig> {
    if cfg!(debug_assertions) {
        // try a few times then panic instead of blocking
        for _ in 0..3 {
            match CONFIG.try_lock() {
                Some(guard) => return guard,
                None => continue,
            }
        }

        panic!("config lock is already held")
    } else {
        CONFIG.lock()
    }
}

pub fn init<P: AsRef<Path>>(path: P) -> ConfigResult<()> {
    let path = path.as_ref().canonicalize().map_err(ConfigError::Io)?;

    if !path.is_file() {
        return Err(ConfigError::NotAFile);
    }

    let mut raw = lock();
    raw.path = Some(path.clone());

    // watch directory for changes
    let watch_dir = path.parent().expect("file should have a parent dir");
    let watch_file = path.file_name().map(|s| s.to_owned()).unwrap();

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).map_err(ConfigError::Notify)?;
    watcher
        .watch(watch_dir, RecursiveMode::NonRecursive)
        .map_err(ConfigError::Notify)?;
    raw.watcher = Some(watcher);

    // start watcher thread
    thread::Builder::new()
        .name("cfg-watcher".to_owned())
        .spawn(move || {
            let channel = rx;
            let is_config = |p: &PathBuf| p.file_name().map(|f| f == watch_file).unwrap_or(false);

            loop {
                let reload = match channel.recv() {
                    Ok(e) => match e {
                        DebouncedEvent::Write(ref p) if is_config(p) => true,
                        DebouncedEvent::Remove(ref p) if is_config(p) => {
                            warn!("config was deleted");
                            true
                        }
                        DebouncedEvent::Rename(ref a, ref b) if is_config(a) || is_config(b) => {
                            warn!("config was renamed");
                            true
                        }
                        _ => false,
                    },
                    Err(e) => {
                        warn!("error while watching config"; "error" => %e);
                        continue;
                    }
                };

                if reload {
                    info!("config was modified, reloading");
                    if let Err(e) = lock().parse_file() {
                        warn!("failed to reload config"; "error" => %e);
                    }
                }
            }
        })
        .expect("failed to start watcher thread");

    raw.parse_file()
}

// TODO add a variant that returns a default instead of panicking
pub fn get<'a>() -> ConfigRef<'a> {
    let guard = lock();
    if guard.parsed.as_ref().is_some() {
        ConfigRef { config: guard }
    } else {
        // intentional panic - this only happens if the config fails on its initial load
        panic!("config must be loaded")
    }
}

pub fn load_time() -> Instant {
    lock().load_time
}
