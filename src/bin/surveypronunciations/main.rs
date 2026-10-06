
use std::{vec::Vec, io::{BufReader, BufRead}, collections::HashMap, fs::File};

fn main() {
  let pronunciation_file = "res/topwords-pronunciation.txt";
  
  let mut pronunciation_table: HashMap<String, Vec<String>> = HashMap::new();
  
  for line in BufReader::new(File::open(pronunciation_file).unwrap()).lines() {
    let line = line.unwrap();
    let tokens: Vec<&str> = line.split(" ").collect();
    let word = tokens[0].to_owned();
    let pronunciation: Vec<String> = tokens[1..].iter().map(|t| (*t).to_owned()).collect();
    pronunciation_table.insert(word, pronunciation);
  }
  
  let test = |w: &str| {
    println!("{}: {:?}", w, pronunciation_table.get(w));
  };
  
  test("hiv")
}

