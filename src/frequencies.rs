
use std::collections::{HashMap, BTreeMap};
use std::fs;

use serde::{Serialize, Deserialize};

use crate::dictionary::load_dictionary;
use crate::glyphs::Glyph;

#[derive(Serialize, Deserialize)]
struct StorageTableEntry {
  frequency: f64,
  associated_sound_frequencies: Vec<(Glyph, f64)>
}

#[derive(Serialize, Deserialize)]
struct FrequencyTableStorage {
  items: Vec<(Vec<Glyph>, StorageTableEntry)>
}

pub struct FrequencyTable {
  pub items: HashMap<Vec<Glyph>, TableEntry>
}

impl FrequencyTable {
  fn to_storage(self) -> FrequencyTableStorage {
    FrequencyTableStorage {
      items: self.items.into_iter().map(|(k, e)| (k, StorageTableEntry { frequency: e.frequency, associated_sound_frequencies: e.associated_sound_frequencies.into_iter().collect() }) ).collect()
    }
  }
  
  fn from_storage(storage: FrequencyTableStorage) -> FrequencyTable {
    FrequencyTable {
      items: storage.items.into_iter().map(|(k, e)| (k, TableEntry { frequency: e.frequency, associated_sound_frequencies: e.associated_sound_frequencies.into_iter().collect() } )).collect()
    }
  }
}

pub struct TableEntry {
  pub frequency: f64,
  pub associated_sound_frequencies: BTreeMap<Glyph, f64>
}

pub fn build_frequency_table() -> FrequencyTable {
  let dictionary = load_dictionary().unwrap();
  
  let mut frequency_table: HashMap<Vec<Glyph>, TableEntry> = HashMap::new();
  
  for word in &dictionary.words {
    let max_n = if word.spelling.len() > 5 { 5 } else { word.spelling.len() };
    for n in 1 .. max_n+1 {
      for start_i in 0 .. word.spelling.len()+1-n {
        let key = word.spelling[start_i .. start_i + n].to_vec();
        if !frequency_table.contains_key(&key) {
          frequency_table.insert(key.clone(), TableEntry {
            frequency: 0.0,
            associated_sound_frequencies: BTreeMap::new()
          });
        }
        let entry = frequency_table.get_mut(&key).unwrap();
        entry.frequency += word.frequency;
        for g in &word.pronunciation {
          entry.associated_sound_frequencies.insert(g.clone(),
            entry.associated_sound_frequencies.get(g).unwrap_or(&0.0) + word.frequency
          );
        }
      }
    }
  }
  
  FrequencyTable {
    items: frequency_table
  }
}

pub fn build_table_and_save_to_file() {
  let table_file = "res/frequency-table.json";
  serde_json::to_writer(fs::File::create(table_file).unwrap(), &build_frequency_table().to_storage()).unwrap();
}

pub fn load() -> FrequencyTable {
  let table_file = "res/frequency-table.json";
  FrequencyTable::from_storage(serde_json::from_reader(fs::File::open(table_file).unwrap()).unwrap())
}

