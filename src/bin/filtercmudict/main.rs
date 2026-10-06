
use std::{collections::HashSet, io::{self, BufRead, Write}, fs};

fn main() {
  let in_dict_path = "res/cmudict.dict";
  let out_dict_path = "res/topwords-pronunciation.txt";
  let topwords_path = "res/topwords.txt";
  
  let topwords_sel: HashSet<String> = io::BufReader::new(fs::File::open(topwords_path).unwrap())
    .lines().skip(1).map(|line| {
      let line = line.unwrap();
      line.split(" ").nth(1).unwrap().to_owned()
    }).collect();
  
  println!("Collected {} top words", topwords_sel.len());
  
  println!("Is 'three' among them? {}", topwords_sel.contains("three"));
  println!("Is 'cataclysm' among them? {}", topwords_sel.contains("cataclysm"));
  println!("Is 'irritating' among them? {}", topwords_sel.contains("irritating"));
  println!("Is 'annoying' among them? {}", topwords_sel.contains("annoying"));
  println!("Is 'splendid' among them? {}", topwords_sel.contains("splendid"));
  println!("Is 'candid' among them? {}", topwords_sel.contains("candid"));
  println!("Is 'breakfast' among them? {}", topwords_sel.contains("breakfast"));
  println!("Is 'lunch' among them? {}", topwords_sel.contains("lunch"));
  println!("Is 'baby' among them? {}", topwords_sel.contains("baby"));
  println!("Is 'tooth' among them? {}", topwords_sel.contains("tooth"));
  println!("Is 'crying' among them? {}", topwords_sel.contains("crying"));
  println!("Is 'trying' among them? {}", topwords_sel.contains("trying"));
  println!("Is 'being' among them? {}", topwords_sel.contains("being"));
  println!("Is 'going' among them? {}", topwords_sel.contains("going"));
  println!("Is 'feet' among them? {}", topwords_sel.contains("feet"));
  println!("Is 'roads' among them? {}", topwords_sel.contains("roads"));
  println!("Is 'uses' among them? {}", topwords_sel.contains("uses"));
  
  let mut out_writer = io::BufWriter::new(fs::File::create(out_dict_path).unwrap());
  
  for line in io::BufReader::new(fs::File::open(in_dict_path).unwrap()).lines().map(|line|line.unwrap()).filter(|line| {
    let word = line.split(" ").next().unwrap();
    topwords_sel.contains(word)
  }) {
    out_writer.write(line.as_bytes()).unwrap();
    out_writer.write("\n".as_bytes()).unwrap();
  }
}


