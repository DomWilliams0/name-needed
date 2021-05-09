use std::mem::MaybeUninit;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use arc_swap::{ArcSwap, Guard};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};

use common::*;

use crate::config::Config;
use std::borrow::Cow;

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
}

type ConfigResult<T> = std::result::Result<T, ConfigError>;

pub enum ConfigType<'a> {
    String(&'a str),
    WatchedFile(&'a Path),
}

/// Must be initialized by [init] before being accessed. This is *debug* asserted on access
static mut CONFIG: MaybeUninit<ArcSwap<Config>> = MaybeUninit::uninit();
static INITIALIZED: AtomicBool = AtomicBool::new(false);

fn is_initialized() -> bool {
    INITIALIZED.load(Relaxed)
}

/// Must be called once only, and before [get]
pub fn init(cfg: ConfigType) -> ConfigResult<()> {
    assert!(!is_initialized(), "config can only be initialized once");

    // parse config and fail early
    let config = cfg.load()?;

    // watch directory for changes if requested
    if let ConfigType::WatchedFile(path) = cfg {
        let path = path.to_owned();
        let watch_dir = path.parent().expect("file should have a parent dir");
        let watch_file = path.file_name().map(|s| s.to_owned()).unwrap();

        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).map_err(ConfigError::Notify)?;
        watcher
            .watch(watch_dir, RecursiveMode::NonRecursive)
            .map_err(ConfigError::Notify)?;

        // start watcher thread
        thread::Builder::new()
            .name("cfg-watcher".to_owned())
            .spawn(move || {
                let _watcher = watcher; // keep alive
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

                        match ConfigType::WatchedFile(&path).load() {
                            Ok(config) => {
                                assert!(is_initialized());

                                // safety: checked for initialization
                                let cfg = unsafe { &*CONFIG.as_ptr() };

                                let new = Arc::new(config);
                                let new_ptr = Arc::as_ptr(&new);

                                let old = cfg.swap(new);
                                let old_ptr = Arc::as_ptr(&old);

                                debug!("swapped config instance"; "new" => ?new_ptr, "old" => ?old_ptr);
                            }
                            Err(e) => {
                                warn!("failed to reload config"; "error" => %e);
                            }
                        }
                    }
                }
            })
            .expect("failed to start watcher thread");
    }

    // initialize globals
    // safety: checked to be currently uninitialized
    unsafe {
        debug_assert!(!is_initialized()); // sanity check
        let ptr = CONFIG.as_mut_ptr();
        ptr.write(ArcSwap::from_pointee(config));
    }
    INITIALIZED.store(true, Relaxed);

    Ok(())
}

pub fn get() -> impl Deref<Target = Config> {
    debug_assert!(is_initialized(), "config has not been initialized");

    let cfg = unsafe { &*CONFIG.as_ptr() };
    Guard::into_inner(cfg.load())
}

impl<'a> ConfigType<'a> {
    fn load(&self) -> ConfigResult<Config> {
        let bytes = match self {
            ConfigType::String(s) => Cow::Borrowed(*s),
            ConfigType::WatchedFile(path) => {
                let contents = std::fs::read_to_string(*path).map_err(ConfigError::Io)?;
                Cow::Owned(contents)
            }
        };

        ron::de::from_str(&bytes).map_err(ConfigError::Parsing)
    }
}
