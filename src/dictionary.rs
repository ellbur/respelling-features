
use crate::glyphs::*;
use lazy_static::lazy_static;
use regex::Regex;
use std::{vec::Vec, io::{self, BufReader, BufRead}, collections::HashMap, fs::File};
use std::fs;

#[derive(Clone)]
pub struct Dictionary {
  pub words: Vec<DictionaryWord>
}

#[derive(Clone)]
pub struct DictionaryWord {
  pub spelling: Vec<Glyph>,
  pub pronunciation: Vec<Glyph>,
  pub frequency: f64
}

pub fn pronunciation_token_to_glyph(t: &str) -> Glyph {
  use Glyph::*;
  
  lazy_static! {
    static ref DIGIT_STRIPPING_RE: Regex = Regex::new(r"\d+").unwrap();
  }

  let without_digits = {
    DIGIT_STRIPPING_RE.replace(t, "")
  };
  
  if      without_digits == "AH" { Uh }
  else if without_digits == "EY" { Ei }
  else if without_digits == "AA" { Ah }
  else if without_digits == "OW" { O }
  else if without_digits == "UH" { Eu }
  else if without_digits == "AE" { Ae }
  else if without_digits == "EH" { Eh }
  else if without_digits == "ZH" { Jh }
  else if without_digits == "IY" { Ee }
  else if without_digits == "IH" { Ih }
  else if without_digits == "ER" { Er }
  else if without_digits == "DH" { Dh }
  else if without_digits == "JH" { J }
  else if without_digits == "UW" { U }
  else if without_digits == "HH" { H }
  else if without_digits == "AO" { Aw }
  else if without_digits == "AW" { Ow }
  else if without_digits == "AY" { I }
  else if without_digits == "OY" { Oi }
  
  else if without_digits == "TH" { Th }
  else if without_digits == "CH" { Ch }
  else if without_digits == "SH" { Sh }
  else if without_digits == "NG" { Ng }
  
  else if without_digits == "B" { B }
  else if without_digits == "D" { D }
  else if without_digits == "F" { F }
  else if without_digits == "G" { G }
  else if without_digits == "H" { H }
  else if without_digits == "J" { J }
  else if without_digits == "K" { K }
  else if without_digits == "L" { L }
  else if without_digits == "M" { M }
  else if without_digits == "N" { N }
  else if without_digits == "P" { P }
  else if without_digits == "R" { R }
  else if without_digits == "S" { S }
  else if without_digits == "T" { T }
  else if without_digits == "V" { V }
  else if without_digits == "W" { W }
  else if without_digits == "Y" { Y }
  else if without_digits == "Z" { Z }
  
  else {
    panic!("Unrecognized sound: {}", t)
  }
}

fn letter_to_glyph(letter: &char) -> Glyph {
  use Glyph::*;

  match letter {
    'a' => A, 'b' => B, 'c' => C, 'd' => D, 'e' => E, 'f' => F, 'g' => G,
    'h' => H, 'i' => I, 'j' => J, 'k' => K, 'l' => L, 'm' => M, 'n' => N,
    'o' => O, 'p' => P, 'q' => Q, 'r' => R, 's' => S, 't' => T, 'u' => U,
    'v' => V, 'w' => W, 'x' => X, 'y' => Y, 'z' => Z,
    '-' => Hyphen, '\'' => Apos,
    _ => panic!("Unrecognized letter: {}", letter)
  }
}

pub fn load_dictionary() -> io::Result<Dictionary> {
  let frequency_file = "res/topwords.txt";
  let pronunciation_file = "res/topwords-pronunciation-2.txt";
  
  let pronunciation_table: HashMap<String, Vec<Glyph>> = io::BufReader::new(fs::File::open(pronunciation_file).unwrap()).lines().map(|line| {
    let line = line.unwrap();
    let mut iterator = line.split(" ");
    let word = iterator.next().unwrap().trim().to_owned();
    let pronunciation = decode(iterator.next().unwrap().trim());
    (word, pronunciation)
  }).collect();
  
  let mut words: Vec<DictionaryWord> = Vec::new();
  
  for line in BufReader::new(File::open(frequency_file)?).lines().skip(1) {
    let line = line?;
    let tokens: Vec<&str> = line.split(" ").skip(1).take(2).collect();
    
    let word = tokens[0];
    let freq_str = tokens[1];
    
    if let Some(pronunciation) = pronunciation_table.get(word) {
      let pronunciation = pronunciation.clone();
      let spelling: Vec<Glyph> = word.chars().map(|c| letter_to_glyph(&c)).collect();
      let frequency: i32 = freq_str.parse().unwrap();
      let frequency = frequency as f64;
      
      words.push(DictionaryWord { spelling, pronunciation, frequency });
    }
  }
  
  let top_freq = words[0].frequency;
  for w in &mut words {
    w.frequency /= top_freq;
  }
  
  Ok(Dictionary {
    words
  })
}

pub fn load_old_dictionary() -> io::Result<Dictionary> {
  let frequency_file = "res/topwords.txt";
  let pronunciation_file = "res/topwords-pronunciation.txt";
  
  let mut pronunciation_table: HashMap<String, Vec<Glyph>> = HashMap::new();
  
  lazy_static! {
    static ref COMMENT_STRIPPING_RE: Regex = Regex::new(r"\s*\#.*").unwrap();
  }

  for line in BufReader::new(File::open(pronunciation_file)?).lines() {
    let line = line?;
    let line = COMMENT_STRIPPING_RE.replace(&line, "");
    let tokens: Vec<&str> = line.split(" ").collect();
    let word = tokens[0].to_owned();
    let pronunciation: Vec<Glyph> = tokens[1..].iter().map(|t| pronunciation_token_to_glyph(t)).collect();
    pronunciation_table.insert(word, pronunciation);
  }
  
  pronunciation_table.insert("a".to_owned(), vec![Glyph::A]);
  
  let mut words: Vec<DictionaryWord> = Vec::new();
  
  for line in BufReader::new(File::open(frequency_file)?).lines().skip(1) {
    let line = line?;
    let tokens: Vec<&str> = line.split(" ").skip(1).take(2).collect();
    
    let word = tokens[0];
    let freq_str = tokens[1];
    
    if let Some(pronunciation) = pronunciation_table.get(word) {
      let pronunciation = pronunciation.clone();
      let spelling: Vec<Glyph> = word.chars().map(|c| letter_to_glyph(&c)).collect();
      let frequency: i32 = freq_str.parse().unwrap();
      let frequency = frequency as f64;
      
      words.push(DictionaryWord { spelling, pronunciation, frequency });
    }
  }
  
  let top_freq = words[0].frequency;
  for w in &mut words {
    w.frequency /= top_freq;
  }
  
  Ok(Dictionary {
    words
  })
}

