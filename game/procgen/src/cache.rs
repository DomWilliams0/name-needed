use std::fs::OpenOptions;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use common::*;

use crate::continent::ContinentMap;
use crate::planet::PlanetInner;
use crate::PlanetParams;

pub fn save(planet: &PlanetInner) -> BoxedResult<()> {
    let path = cache_file(&planet.params);
    info!("caching planet to {file}", file = path.display());
    std::fs::create_dir_all(path.parent().unwrap())?;

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(&path)?;

    // TODO cache global features too
    bincode::serialize_into(&mut file, &planet.continents)?;

    Ok(())
}

pub fn try_load(params: &PlanetParams) -> BoxedResult<Option<ContinentMap>> {
    let path = cache_file(params);
    debug!("checking for cache"; "file" => path.display());

    if !path.is_file() {
        // not cached
        return Ok(None);
    }

    let file = OpenOptions::new().read(true).open(&path)?;
    bincode::deserialize_from(file)
        .map(Some)
        .map_err(Into::into)
}

fn cache_file(params: &PlanetParams) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("nn-procgen-cache");
    path.push(hash(params));
    path.set_extension("cache");
    path
}

fn hash(params: &PlanetParams) -> String {
    let mut input = Vec::new();
    bincode::serialize_into(&mut input, params).expect("failed to serialize");

    let hash = Sha256::digest(&input);
    format!("{:x}", hash)
}
