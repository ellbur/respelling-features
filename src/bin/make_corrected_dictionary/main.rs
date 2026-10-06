
use std::{collections::HashSet, io::{self, BufRead, Write}, fs};
use std::collections::{HashMap, hash_map::Entry::*};
use feature_refining::{glyphs::{Glyph, encode, decode}, dictionary::pronunciation_token_to_glyph};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
  static ref COMMENT_STRIPPING_RE: Regex = Regex::new(r"\s*\#.*").unwrap();
}

fn main() {
  let in_dict_path = "res/cmudict.dict";
  let in_choices_path = "res/decided-pronunciations.txt";
  let out_dict_path = "res/topwords-pronunciation-2.txt";
  let topwords_path = "res/topwords.txt";
  
  let topwords_sel: HashSet<String> = io::BufReader::new(fs::File::open(topwords_path).unwrap())
    .lines().skip(1).map(|line| {
      let line = line.unwrap();
      line.split(" ").nth(1).unwrap().to_owned()
    }).collect();
  
  println!("Collected {} top words", topwords_sel.len());
  
  println!("Is 'three' among them? {}", topwords_sel.contains("three"));
  println!("Is 'cataclysm' among them? {}", topwords_sel.contains("cataclysm"));
  println!("");
  
  let mut dict: HashMap<String, Vec<Glyph>> = io::BufReader::new(fs::File::open(in_choices_path).unwrap()).lines().map(|line| {
    let line = line.unwrap();
    let mut iterator = line.split(" ");
    let word = iterator.next().unwrap().trim().to_owned();
    let pronunciation = decode(iterator.next().unwrap().trim());
    (word, pronunciation)
  }).collect();
  
  for line in io::BufReader::new(fs::File::open(in_dict_path).unwrap()).lines().map(|line|line.unwrap()) {
    let line = COMMENT_STRIPPING_RE.replace(&line, "");
    let word_and_number = line.split(" ").next().unwrap();
    let word = word_and_number.split("(").next().unwrap().to_owned();
    let tokens: Vec<&str> = line.split(" ").collect();
    let pronunciation: Vec<Glyph> = tokens[1..].iter().map(|t| pronunciation_token_to_glyph(t)).collect();
    if topwords_sel.contains(&word) {
      match dict.entry(word) {
        Vacant(entry) => {
          entry.insert(pronunciation);
        },
        Occupied(_) => ()
      }
    }
  }
  
  let mut out_writer = io::BufWriter::new(fs::File::create(out_dict_path).unwrap());
  
  let mut dict: Vec<_> = dict.into_iter().collect();
  dict.sort();
  for (word, pronunciation) in dict {
    out_writer.write(format!("{} {}\n", word, encode(&pronunciation)).as_bytes()).unwrap();
  }
}

