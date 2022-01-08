use lzma_rs::lzma_decompress;
use resources::{ReadResource, ResourcePath};
use smol_str::SmolStr;
use std::borrow::Borrow;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

const MAX_LEN: usize = 6;

#[derive(Default)]
pub struct SourceWords {
    words: Box<[SmolStr]>,
}

impl SourceWords {
    pub fn load_resource(res: &ResourcePath) -> Result<SourceWords, Box<dyn Error>> {
        let in_file = Vec::<u8>::read_resource(res)?;
        Self::load_from_reader(BufReader::new(Cursor::new(in_file)))
    }

    pub fn load_path(path: &Path) -> Result<SourceWords, Box<dyn Error>> {
        let reader = OpenOptions::new()
            .read(true)
            .open(path)
            .map(BufReader::new)?;

        Self::load_from_reader(reader)
    }

    fn load_from_reader(mut reader: impl BufRead) -> Result<SourceWords, Box<dyn Error>> {
        fn load_decompressed(input: Vec<u8>) -> Result<SourceWords, Box<dyn Error>> {
            let mut all = Vec::with_capacity(128);

            let mut i = 0;
            loop {
                let line = &input[i..];
                let next = match line.iter().position(|b| *b == b'\n') {
                    None => break,
                    Some(i) => i,
                };

                if next <= MAX_LEN {
                    let word = std::str::from_utf8(&line[..next])?.trim_end();
                    all.push(SmolStr::new_inline(word));
                }
                i += next + 1;
            }

            Ok(SourceWords {
                words: all.into_boxed_slice(),
            })
        }

        let mut bytes = Vec::new();
        lzma_decompress(&mut reader, &mut bytes)?;
        load_decompressed(bytes)
    }

    pub fn words(&self) -> &[impl Borrow<str>] {
        &self.words
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> + ExactSizeIterator + '_ {
        self.words.iter().map(|s| s.as_str())
    }
}
