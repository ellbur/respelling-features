
use std::{collections::{HashSet, HashMap, hash_map::Entry::*}, io::{self, BufRead, Write}, fs};
use feature_refining::{glyphs::{Glyph, encode}, dictionary::pronunciation_token_to_glyph};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use dialoguer::Select;
use console::style;

lazy_static! {
  static ref COMMENT_STRIPPING_RE: Regex = Regex::new(r"\s*\#.*").unwrap();
}

fn annotate_changes(pronunciations: &Vec<Vec<Glyph>>) -> Vec<Vec<bool>> {
  pronunciations.iter().enumerate().map(|(i, p_i)| {
    let n = p_i.len();
    let mut res = vec![false; n];
    
    for (j, p_j) in pronunciations.iter().enumerate() {
      if j != i {
        let mut k = 0;
        for item in diff::slice(p_i, p_j) {
          match item {
            diff::Result::Left(_) => {
              res[k] = true;
              k += 1;
            },
            diff::Result::Both(_, _) => {
              k += 1;
            },
            diff::Result::Right(_) => { }
          }
        }
      }
    }
    
    res
  }).collect()
}

fn main() {
  let in_dict_path = "res/cmudict.dict";
  let out_dict_path = "res/decided-pronunciations.txt";
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
  
  let mut decision_set: HashMap<String, HashSet<Vec<Glyph>>> = HashMap::new();
  
  for line in io::BufReader::new(fs::File::open(in_dict_path).unwrap()).lines().map(|line|line.unwrap()) {
    let line = COMMENT_STRIPPING_RE.replace(&line, "");
    let word_and_number = line.split(" ").next().unwrap();
    let word = word_and_number.split("(").next().unwrap().to_owned();
    let tokens: Vec<&str> = line.split(" ").collect();
    let pronunciation: Vec<Glyph> = tokens[1..].iter().map(|t| pronunciation_token_to_glyph(t)).collect();
    if topwords_sel.contains(&word) {
      match decision_set.entry(word) {
        Vacant(entry) => {
          entry.insert(HashSet::from([pronunciation]));
        },
        Occupied(mut entry) => {
          entry.get_mut().insert(pronunciation);
        }
      }
    }
  }
  
  decision_set.retain(|_, ps| ps.len() > 1);
  
  let total_size = decision_set.len();

  let existing: HashSet<String> = match fs::File::open(out_dict_path) {
    Ok(file) => io::BufReader::new(file).lines().map(|line| {
      line.unwrap().split(" ").next().unwrap().to_owned()
    }).collect(),
    Err(_) => HashSet::new()
  };
  
  decision_set.retain(|word, _| !existing.contains(word));
  
  println!("You must make {}/{} decisions.", decision_set.len(), total_size);
  println!("");
  
  let mut out_writer = io::BufWriter::new(fs::OpenOptions::new().create(true).append(true).open(out_dict_path).unwrap());
  
  for (word, ps) in decision_set {
    let mut ps: Vec<Vec<Glyph>> = ps.iter().map(|p| p.clone()).collect();
    ps.sort();
    
    let encodeds: Vec<String> = ps.iter().map(|p| encode(p)).collect();
    
    let changes = annotate_changes(&ps);
    
    let choices: Vec<String> = ps.iter().enumerate().map(|(i, p)| {
      format!("{}", p.iter().enumerate().map(|(k, g)| {
        let text = g.name();
        let text = if g.is_vowel() { format!("{}", style(text).cyan()) } else { text };
        let text = if changes[i][k] { format!("{}", style(text).bold().underlined()) } else { text };
        text
      }).format(""))
    }).collect();
    
    loop {
      if let Ok(selection) = Select::new().with_prompt(word.clone()).items(&choices).interact() {
        out_writer.write(format!("{} {}\n", word, encodeds[selection]).as_bytes()).unwrap();
        break;
      }
    }
    
    out_writer.flush().unwrap();
  }
}

