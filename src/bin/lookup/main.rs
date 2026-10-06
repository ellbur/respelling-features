// Prints the dictionary's pronunciation of each word given, as the rules
// see it (after ipa.rs's conversion).
//
//   lookup [--dictionary readlex-phonemic] word...

use clap::Parser;

use feature_refining::glyphs::{decode, encode};

#[derive(Parser)]
struct Args {
  #[arg(long, default_value = "readlex-phonemic")]
  dictionary: String,
  words: Vec<String>
}

fn main() {
  let args = Args::parse();
  let dictionary = match args.dictionary.as_str() {
    "readlex" => feature_refining::readlex::load_readlex_ipa_american_tweaked_dictionary().unwrap(),
    "readlex-phonemic" => feature_refining::readlex::load_readlex_phonemic_dictionary().unwrap(),
    "cmudict" => feature_refining::dictionary::load_dictionary().unwrap(),
    other => panic!("Unknown dictionary {}", other)
  };
  for word in &args.words {
    let spelling = decode(&word.to_lowercase());
    match dictionary.words.iter().find(|w| w.spelling == spelling) {
      Some(w) => println!("{} {}", word, encode(&w.pronunciation)),
      None => println!("{} -", word)
    }
  }
}
