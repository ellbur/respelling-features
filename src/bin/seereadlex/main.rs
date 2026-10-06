
use std::collections::HashMap;
use json::JsonValue;

#[derive(Debug)]
#[allow(dead_code)]
struct ReadlexEntry {
  shaw: String,
  freq: u32
}

fn main() {
  let readlex = json::parse(&std::fs::read_to_string("res/readlex.json").unwrap()).unwrap();
  
  let JsonValue::Object(obj) = readlex else { panic!("No"); };
  
  let mut dict: HashMap<String, ReadlexEntry> = HashMap::new();
  
  for (_, entry_set) in obj.iter() {
    let JsonValue::Array(entry_set) = entry_set else { panic!("No"); };
    for entry in entry_set.iter() {
      let latin = entry["Latn"].as_str().unwrap().to_string();
      let shaw = entry["Shaw"].as_str().unwrap().to_string();
      let freq = entry["freq"].as_u32().unwrap();
      
      dict.insert(latin, ReadlexEntry {
        shaw,
        freq
      });
    }
  }
  
  println!("Read {} entries", dict.len());
  println!("");
  println!("cat: {:?}", dict.get("cat"));
  println!("the: {:?}", dict.get("the"));
  println!("explain: {:?}", dict.get("explain"));
  println!("toasted: {:?}", dict.get("toasted"));
  println!("beautiful: {:?}", dict.get("beautiful"));
}

