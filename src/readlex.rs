
use float_ord::FloatOrd;
use serde::{Serialize, Deserialize};
use std::io;
use std::fs::{self, File};
use crate::glyphs::Glyph;
use crate::dictionary::{Dictionary, DictionaryWord};
use std::collections::HashMap;
use itertools::Itertools;
use std::io::BufRead;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadlexEntry {
  pub latin: String,
  pub shaw: String,
  pub ipa: String,
  pub freq: f64,
}

pub fn read_readlex_top5000() -> io::Result<Vec<ReadlexEntry>> {
  serde_json::from_reader(
    io::BufReader::new(
      File::open("res/readlex-entries-top5000.json")?
    )
  ).map_err(|e| e.into())
}

pub fn read_readlex_top_american5000() -> io::Result<Vec<ReadlexEntry>> {
  serde_json::from_reader(
    io::BufReader::new(
      File::open("res/readlex-entries-top-american5000.json")?
    )
  ).map_err(|e| e.into())
}

pub fn shaw_char_to_glyphs(sh: char) -> Option<Vec<Glyph>> {
  use Glyph::*;
  Some(match sh {
    '𐑑' => vec![T],
    '𐑔' => vec![Th],
    '𐑩' => vec![Schwa],
    '𐑴' => vec![O],
    '𐑟' => vec![Z],
    '𐑪' => vec![Ah],
    '𐑥' => vec![M],
    '𐑙' => vec![Ng],
    '𐑳' => vec![Uh],
    '𐑐' => vec![P],
    '𐑚' => vec![B],
    '𐑓' => vec![F],
    '𐑝' => vec![V],
    '𐑯' => vec![N],
    '𐑛' => vec![D],
    '𐑤' => vec![L],
    '𐑶' => vec![Oi],
    '𐑦' => vec![Ih],
    '𐑲' => vec![I],
    '𐑕' => vec![S],
    '𐑒' => vec![K],
    '𐑿' => vec![Yu],
    '𐑷' => vec![Aw],
    '𐑼' => vec![Er],
    '𐑖' => vec![Sh],
    '𐑠' => vec![Jh],
    '𐑗' => vec![Ch],
    '𐑡' => vec![J],
    '𐑘' => vec![Y],
    '𐑢' => vec![W],
    '𐑣' => vec![H],
    '𐑮' => vec![R],
    '𐑰' => vec![Ee],
    '𐑧' => vec![Eh],
    '𐑱' => vec![Ei],
    '𐑨' => vec![Ae],
    '𐑫' => vec![Eu],
    '𐑵' => vec![U],
    '𐑬' => vec![Ow],
    '𐑸' => vec![Ah, R],
    '𐑺' => vec![Ei, R],
    '𐑻' => vec![Eh, R],
    '𐑽' => vec![Ee, R],
    '𐑹' => vec![Aw, R],
    '𐑾' => vec![Ee, Eh],
    _ => None?
  })
}

pub fn shaw_word_to_glyphs(sh: &str) -> Vec<Glyph> {
  let mut res = vec![];

  for c in sh.chars() {
    if let Some(gs) = shaw_char_to_glyphs(c) {
      res.extend(gs);
    }
  }

  res
}

pub fn shaw_word_to_glyphs_with_fixes(sh: &str, latin: &str) -> Vec<Glyph> {
  let mut res = vec![];

  for c in sh.chars() {
    if let Some(gs) = shaw_char_to_glyphs(c) {
      res.extend(gs);
    }
  }

  let n = res.len();
  if n > 0 && res[n - 1] == Glyph::Ih {
    res[n - 1] = Glyph::Ee;
  }

  if n > 0 && res[0] == Glyph::Ih {
    let latin_glyphs = crate::glyphs::decode(latin);
    if latin_glyphs.len() > 0 && latin_glyphs[0] == Glyph::E {
      res[0] = Glyph::Schwa;
    }
  }

  res
}

pub fn fix_final_ih(gs: &Vec<Glyph>) -> Vec<Glyph> {
  let mut res = gs.clone();
  let n = res.len();
  if n > 0 && res[n - 1] == Glyph::Ih {
    res[n - 1] = Glyph::Ee;
  }
  res
}

// Hand-decided IPA pronunciations, keyed by spelling, that override ReadLex's.
pub fn load_pronunciation_tweaks() -> HashMap<String, String> {
  let tweaked_path = "res/decided-pronunciations-ipa.txt";

  match fs::File::open(tweaked_path) {
    Ok(file) => io::BufReader::new(file).lines().map(|line| {
      let tokens: Vec<String> = line.unwrap().split(" ").map(|s| s.to_owned()).collect_vec();
      let first = tokens[0].to_owned();
      let second = tokens[1].to_owned();
      (first, second)
    }).collect(),
    Err(_) => HashMap::new()
  }
}

pub struct ReadlexIPATweakedEntry {
  pub latin: String,
  pub ipa: String,
  pub ipa_glyphs: Option<Vec<Glyph>>,
  pub tweaked_ipa: Option<String>,
  pub tweaked_ipa_glyphs: Option<Vec<Glyph>>,
  pub frequency: f64
}

pub fn load_readlex_ipa_american_tweaked_table() -> Result<Vec<ReadlexIPATweakedEntry>, io::Error> {
  use crate::ipa::ipa_to_pronunciation_glyphs;

  let readlex = read_readlex_top_american5000().unwrap();
  let tweaks = load_pronunciation_tweaks();

  let mut entries: Vec<ReadlexIPATweakedEntry> = readlex.into_iter().map(|entry| {
    let ipa = entry.ipa;
    let tweaked_ipa = tweaks.get(&entry.latin).map(|i| i.to_owned());

    let ipa_glyphs = ipa_to_pronunciation_glyphs(&ipa).ok();
    let tweaked_ipa_glyphs = tweaked_ipa.clone().and_then(|ipa| ipa_to_pronunciation_glyphs(&ipa).ok());

    ReadlexIPATweakedEntry {
      latin: entry.latin.to_string(),
      ipa,
      ipa_glyphs,
      tweaked_ipa,
      tweaked_ipa_glyphs,
      frequency: entry.freq
    }
  }).collect();

  entries.sort_by_key(|e| FloatOrd(-e.frequency));

  Ok(entries)
}

pub fn load_readlex_ipa_american_tweaked_dictionary() -> Result<Dictionary, io::Error> {
  use crate::ipa::ipa_to_pronunciation_glyphs;

  let readlex = read_readlex_top_american5000().unwrap();
  let tweaks = load_pronunciation_tweaks();

  let mut entries: Vec<DictionaryWord> = readlex.into_iter().map(|entry| {
    let pronunciation_ipa: String = match tweaks.get(&entry.latin) {
      None => {
        entry.ipa
      },
      Some(tweaked_pronunciation) => {
        tweaked_pronunciation.to_owned()
      }
    };

    let spelling = crate::glyphs::decode(&entry.latin);

    let pronunciation = ipa_to_pronunciation_glyphs(&pronunciation_ipa).unwrap();

    DictionaryWord {
      spelling,
      pronunciation,
      frequency: entry.freq
    }
  }).collect();

  entries.sort_by_key(|e| FloatOrd(-e.frequency));

  Ok(Dictionary {
    words: entries
  })
}

// The same words as load_readlex_ipa_american_tweaked_dictionary, with the
// pronunciations in res/pronunciations-phonemic-ipa.txt (see the
// makephonemic program), which treat unstressed vowels consistently.
pub fn load_readlex_phonemic_dictionary() -> Result<Dictionary, io::Error> {
  use crate::ipa::ipa_to_pronunciation_glyphs;

  let pronunciations: HashMap<String, String> = fs::read_to_string("res/pronunciations-phonemic-ipa.txt")?
    .lines()
    .filter_map(|line| line.split_once(' ').map(|(w, ipa)| (w.to_owned(), ipa.to_owned())))
    .collect();

  let mut entries: Vec<DictionaryWord> = read_readlex_top_american5000()?.into_iter().map(|entry| {
    let ipa = pronunciations.get(&entry.latin).unwrap_or_else(|| panic!("no phonemic pronunciation for {}", entry.latin));
    DictionaryWord {
      spelling: crate::glyphs::decode(&entry.latin),
      pronunciation: ipa_to_pronunciation_glyphs(ipa).unwrap(),
      frequency: entry.freq
    }
  }).collect();

  entries.sort_by_key(|e| FloatOrd(-e.frequency));

  Ok(Dictionary {
    words: entries
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_shaw_word_to_glyphs() {
    use Glyph::*;
    assert_eq!(shaw_word_to_glyphs("𐑒𐑸"), vec![K, Ah, R]);
    assert_eq!(shaw_word_to_glyphs("𐑒𐑸-𐑒𐑸"), vec![K, Ah, R, K, Ah, R]);
  }
}

