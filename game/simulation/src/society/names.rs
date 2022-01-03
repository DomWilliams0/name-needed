// TODO name generation per society

use common::*;
use resources::{ResourceContainer, Resources};
use std::borrow::Borrow;
use std::error::Error;

/// Resource for generating names
#[derive(Default)]
pub struct NameGeneration(markov::SourceWords);

impl NameGeneration {
    pub fn generate(&self, rand: &mut dyn RngCore) -> &str {
        self.0
            .words()
            .choose(rand)
            .expect("no source names loaded")
            .borrow()
    }

    pub fn load(res: &Resources) -> Result<Self, Box<dyn Error>> {
        let path = res.get_file("names.txt.lzma")?;
        let source = markov::SourceWords::load_resource(&path)?;
        debug!("loaded {} names", source.words().len());
        Ok(Self(source))
    }
}
