use markov::SourceWords;
use std::path::Path;

pub fn main() {
    let path = std::env::args_os().nth(1).expect("pass file as first arg");
    let loaded = SourceWords::load_path(Path::new(&path)).expect("failed");

    let words = loaded.words();
    eprintln!("loaded {} words", words.len());
}
