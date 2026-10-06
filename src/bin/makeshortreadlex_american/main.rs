
use std::collections::{HashMap, HashSet};
use json::JsonValue;
use itertools::Itertools;
use feature_refining::readlex::ReadlexEntry;

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
  ipa: String
}

// The pronunciation of a possessive ("women's") or "'ve" form ("would've")
// from its base word's, following ReadLex's conventions for plural endings.
// Uses the hand-decided pronunciation of the base word if there is one.
fn derive_apostrophe_form(word: &str, groups: &HashMap<String, ReadlexEntryVariantGroup>, tweaks: &HashMap<String, String>) -> Option<ReadlexEntryVariantGroup> {
  let (base_latin, is_ve) = match word.strip_suffix("'ve") {
    Some(base) => (base, true),
    None => (word.strip_suffix("'s")?, false)
  };
  let base = groups.get(base_latin)?;
  let base_ipa = tweaks.get(base_latin).unwrap_or(&base.ipa);

  let (shaw_suffix, ipa_suffix) =
    if is_ve {
      ("𐑩𐑝", "əv")
    }
    else {
      match base_ipa.chars().last()? {
        's' | 'z' | 'ʃ' | 'ʒ' => ("𐑩𐑟", "Əz"),
        'p' | 't' | 'k' | 'f' | 'θ' => ("𐑕", "s"),
        _ => ("𐑟", "z")
      }
    };

  Some(ReadlexEntryVariantGroup {
    latin: word.to_owned(),
    shaw: format!("{}{}", base.shaw, shaw_suffix),
    ipa: format!("{}{}", base_ipa, ipa_suffix)
  })
}

fn main() {
  let american_frequencies = feature_refining::american_frequencies::load_american_frequencies().unwrap();

  let mut american_frequency_table: HashMap<String, f64> = american_frequencies.clone().into_iter().collect();

  // topwords.txt splits contractions apart, so their frequencies come from a
  // separate file. Only fill in words topwords.txt doesn't already have.
  let contraction_frequencies = feature_refining::american_frequencies::load_contraction_frequencies().unwrap();
  for (word, freq) in contraction_frequencies.iter() {
    american_frequency_table.entry(word.clone()).or_insert(*freq);
  }
  let american_frequency_table = american_frequency_table;

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
          freq,
          ipa,
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

      ReadlexEntryVariantGroup {
        latin,
        shaw: most_common_shaw,
        ipa: most_common_ipa
      }
    });

  // Possessives and "'ve" forms aren't in ReadLex; derive them from the base word.
  let tweaks = feature_refining::readlex::load_pronunciation_tweaks();
  let mut entry_groups: HashMap<String, ReadlexEntryVariantGroup> = entry_groups.map(|g| (g.latin.clone(), g)).collect();
  for (word, _) in contraction_frequencies.iter() {
    if entry_groups.contains_key(word) {
      continue;
    }
    let derived = derive_apostrophe_form(word, &entry_groups, &tweaks);
    if let Some(group) = derived {
      entry_groups.insert(word.clone(), group);
    }
    else {
      println!("Can't derive a pronunciation for {}", word);
    }
  }
  let entry_groups = entry_groups.into_values();

  let entries: Vec<ReadlexEntry> = entry_groups.into_iter().flat_map(
    |ReadlexEntryVariantGroup { latin, shaw, ipa }| {
      american_frequency_table.get(&latin).map(|freq| {
        let freq = *freq;
        ReadlexEntry { latin, shaw, freq, ipa }
      })
    }
  ).collect();
  
  let used_latins: HashSet<String> = entries.iter().map(|e| e.latin.clone()).collect();
  let all_top_latins: HashSet<String> = american_frequencies.iter().map(|e| e.0.clone()).collect();
  let unused_top_latins: HashSet<String> = all_top_latins.difference(&used_latins).map(|w| w.clone()).collect();

  println!("Missing: {:?}", unused_top_latins);

  let mut entries = entries;
  // Break ties by spelling so the output doesn't depend on hash map order.
  entries.sort_by(|a, b| b.freq.partial_cmp(&a.freq).unwrap().then_with(|| a.latin.cmp(&b.latin)));
  
  entries.truncate(5000);
  let entries = entries;
  
  let file = std::fs::File::create("res/readlex-entries-top-american5000.json").unwrap();
  let writer = std::io::BufWriter::new(file);
  serde_json::to_writer(writer, &entries).unwrap();

  println!("Written.");
}

