use std::ops::Deref;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use log::*;
use notify::{watcher, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

use lazy_static::lazy_static;

use crate::config::Config;

type ConfigResult<T> = std::result::Result<T, String>;

#[derive(Default)]
pub(crate) struct RawConfig {
    pub parsed: Option<Config>,
    pub path: Option<PathBuf>,

    watcher: Option<RecommendedWatcher>,
}

impl RawConfig {
    pub fn parse_file(&mut self) -> ConfigResult<()> {
        let path = self.path
            .as_ref()
            .ok_or_else(|| "config path has not been set".to_string())?;
        let bytes =
            std::fs::read_to_string(path.as_path()).map_err(|e| format!("loading config: {}", e))?;
        self.parse_bytes(&bytes)
    }

    pub fn parse_bytes(&mut self, bytes: &str) -> ConfigResult<()> {
        let parsed = ron::de::from_str(bytes).map_err(|e| format!("parsing config: {}", e))?;
        self.parsed = Some(parsed);

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

pub fn init<P: Into<PathBuf>>(path: P) -> ConfigResult<()> {
    let path = path.into()
        .canonicalize()
        .map_err(|e| format!("invalid path: {}", e))?;

    if !path.is_file() {
        return Err("not a file".to_owned());
    }

    let mut raw = CONFIG.lock().unwrap();
    raw.path = Some(path.clone());

    // watch directory for changes
    let watch_dir = path.parent()
        .ok_or_else(|| "invalid config file".to_owned())?;
    let watch_file = path.file_name().map(|s| s.to_owned()).unwrap();

    let (tx, rx) = channel();
    let mut watcher = watcher(tx, Duration::from_secs(1))
        .map_err(|e| format!("failed to setup config watcher: {}", e))?;
    watcher
        .watch(watch_dir, RecursiveMode::NonRecursive)
        .map_err(|e| format!("failed to setup config watcher: {}", e))?;
    raw.watcher = Some(watcher);

    // start watcher thread
    thread::spawn(move || {
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
                    warn!("error while watching config: {}", e);
                    continue;
                }
            };

            if reload {
                info!("config was modified, reloading");
                if let Err(e) = CONFIG.lock().unwrap().parse_file() {
                    warn!("failed to reload config: {}", e);
                }
            }
        }
    });

    raw.parse_file()
}

pub fn get<'a>() -> ConfigRef<'a> {
    let guard = CONFIG.lock().unwrap();
    if guard.parsed.as_ref().is_some() {
        ConfigRef { config: guard }
    } else {
        // intentional panic - this only happens if the config fails on its initial load
        panic!("config must be loaded")
    }
}
