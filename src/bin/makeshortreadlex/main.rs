
use itertools::Itertools;
use json::JsonValue;
use feature_refining::readlex::ReadlexEntry;
use std::collections::HashMap;

#[derive(PartialEq, PartialOrd, Ord, Eq, Hash)]
struct LatinPOS {
  latin: String,
  pos: String
}

#[derive(Debug)]
struct ReadlexEntryVariant {
  shaw: String,
  freq: u32,
  rank: usize,
  ipa: String
}

#[derive(Debug)]
struct ReadlexEntryVariantGroup {
  latin: String,
  shaw: String,
  freq: u32,
  ipa: String
}

fn main() {
  let preference_order = [
    "GenAm", "RRP", "RRPVar", "GenAus", "SSB", "TrapBath"
  ];
  
  let preference_keys: HashMap<&str, usize> = preference_order
    .iter().enumerate().map(|(i, n)| (*n, i)).collect();
  
  let mut entry_pos_map: HashMap<LatinPOS, ReadlexEntryVariant> = HashMap::new();

  let readlex = json::parse(&std::fs::read_to_string("res/readlex.json").unwrap()).unwrap();
  
  let JsonValue::Object(obj) = readlex else { panic!("No"); };
  
  for (_, entry_set) in obj.iter() {
    let JsonValue::Array(entry_set) = entry_set else { panic!("No"); };
    for entry in entry_set.iter() {
      let latin = entry["Latn"].as_str().unwrap().to_string();

      if latin.contains(" ") || latin.contains("-") {
        continue;
      }

      let shaw = entry["Shaw"].as_str().unwrap().to_string();
      let ipa = entry["ipa"].as_str().unwrap().to_string();
      let pos = entry["pos"].as_str().unwrap().to_string();
      let freq = entry["freq"].as_u32().unwrap();
      
      let var = entry["var"].as_str().unwrap();
      
      let rank = *preference_keys.get(var).unwrap();
      
      let latin_pos = LatinPOS { latin: latin.clone(), pos };

      let better =
        match entry_pos_map.get(&latin_pos) {
          Some(ReadlexEntryVariant {rank: old_rank, ..}) => rank < *old_rank,
          None => true
        };
      
      if better {
        entry_pos_map.insert(latin_pos, ReadlexEntryVariant {
          shaw,
          ipa,
          freq,
          rank
        });
      }
    }
  }
  
  let entry_groups = entry_pos_map.into_iter()
    .map(|(LatinPOS {latin, pos: _}, entry)| (latin, entry))
    .into_grouping_map()
    .collect::<Vec<ReadlexEntryVariant>>()
    .into_iter()
    .map(|(latin, group)| {
      let most_common = group.iter().max_by_key(|e| e.freq).unwrap();
      let most_common_shaw = most_common.shaw.clone();
      let most_common_ipa = most_common.ipa.clone();
      let total_freq = group.iter().map(|e| e.freq).sum();

      ReadlexEntryVariantGroup {
        latin,
        shaw: most_common_shaw,
        ipa: most_common_ipa,
        freq: total_freq,
      }
    });
  
  let mut entries: Vec<ReadlexEntry> = entry_groups.into_iter().map(
    |ReadlexEntryVariantGroup { latin, shaw, freq, ipa }| {
      let freq = freq as f64;
      ReadlexEntry { latin, shaw, freq, ipa }
    }
  ).collect();
  
  // Ties in alphabetical order, so the output doesn't depend on hash order.
  entries.sort_by_key(|e| (-(e.freq as i32), e.latin.clone()));
  
  entries.truncate(5000);
  
  let file = std::fs::File::create("res/readlex-entries-top5000.json").unwrap();
  let writer = std::io::BufWriter::new(file);
  serde_json::to_writer(writer, &entries).unwrap();
}

