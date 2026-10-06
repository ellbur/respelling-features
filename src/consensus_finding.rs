
use crate::glyphs::{Glyph, AugGlyph};
use AugGlyph::*;
use crate::high_level_substitutions::{HLSubstitution, HLSubstitutionList, hl_to_ll};
use levenshtein_diff as levenshtein;
use crate::substitutions2::apply_all;
use crate::dictionary::Dictionary;
use std::collections::HashMap;
use float_ord::FloatOrd;

fn distance(p1: &Vec<Glyph>, p2: &Vec<Glyph>) -> u32 {
  levenshtein::distance(p1, p2).0 as u32
}

fn apply_copied(v: &Vec<Glyph>, hl_slist: &HLSubstitutionList) -> Vec<Glyph> {
  let slist = hl_to_ll(hl_slist).unwrap();
  let mut working = v.iter().map(|g| Real(g.clone())).collect();
  apply_all(&mut working, &slist);
  working.into_iter().map(|g| match g {
    Real(g) => g,
    Synthetic(_) => panic!("Leftover synthetic glyphs")
  }).collect()
}

pub fn find_improving_edits(word: &Vec<Glyph>, pronunciation: &Vec<Glyph>, init_hl_slist: &HLSubstitutionList) -> Vec<(HLSubstitution, u32)> {
  let mut res = vec![];
  
  let base_transformed = apply_copied(word, init_hl_slist);
  let base_distance = distance(&base_transformed, pronunciation);
  
  let mut working_hl_slist = init_hl_slist.clone();
  working_hl_slist.substitutions.insert(0, HLSubstitution {
    key: vec![],
    sub_start: 0,
    sub_end: 0,
    at_start: false,
    at_end: false,
    sub_content: vec![]
  });
  
  for k1 in 0 .. word.len() {
    for k2 in (k1+1) .. (word.len()+1) {
      let key_size = k2 - k1;
      for s1 in 0 .. key_size {
        for s2 in (s1+1) .. (key_size+1) {
          for sc1 in 0 .. pronunciation.len() {
            for sc2 in (sc1+1) .. (pronunciation.len()+1) {
              for &at_start in if k1 == 0 {[true, false].iter()} else {[false].iter()} {
                for &at_end in if k2 == word.len() {[true, false].iter()} else {[false].iter()} {
                  let hlsub = HLSubstitution {
                    key: word[k1 .. k2].to_vec(),
                    sub_start: s1,
                    sub_end: s2,
                    at_start,
                    at_end,
                    sub_content: pronunciation[sc1 .. sc2].to_vec()
                  };
                  working_hl_slist.substitutions[0] = hlsub;
                  let new_transformed = apply_copied(word, &working_hl_slist);
                  let new_distance = distance(&new_transformed, pronunciation);
                  if new_distance < base_distance {
                    res.push((working_hl_slist.substitutions[0].clone(), base_distance - new_distance));
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  
  res
}

pub struct WordEntry {
  pub substitutions: Vec<(HLSubstitution, u32)>,
  pub frequency: f64
}

pub struct JustificationTable {
  pub by_word: HashMap<Vec<Glyph>, WordEntry>,
  pub by_sub: HashMap<HLSubstitution, f64>
}

fn sub_size(sub: &HLSubstitution) -> usize {
  sub.key.len() + (if sub.at_start { 1 } else { 0 }) + (if sub.at_end { 1 } else { 0 })
}

pub fn build_justification_table(dictionary: &Dictionary) -> JustificationTable {
  //use rayon::prelude::*;
  
  let by_word_entries: Vec<(Vec<Glyph>, WordEntry)> = dictionary.words.iter().map(|dictionary_word| {
    (
      dictionary_word.spelling.clone(),
      WordEntry {
        substitutions:
          find_improving_edits(
            &dictionary_word.spelling,
            &dictionary_word.pronunciation, 
            &HLSubstitutionList { substitutions: vec![] }
          ),
        frequency: dictionary_word.frequency
      }
    )
  }).collect();
  
  let mut by_sub: HashMap<HLSubstitution, f64> = HashMap::new();
  
  for (_, word_entry) in &by_word_entries {
    for (sub, improvement) in &word_entry.substitutions {
      // The purpose of this fudge factor is not so much to choose better edits as to make
      // the ordering more deterministic and reproducible.
      let fudge_score = 1.0 + 0.11*(*improvement as f64) - 0.09*(sub_size(sub) as f64);
      let amount = word_entry.frequency * fudge_score;
      if let Some(c) = by_sub.get_mut(&sub) {
        *c += amount;
      }
      else {
        by_sub.insert(sub.clone(), amount);
      }
    }
  }
  
  let by_word = HashMap::from_iter(by_word_entries.into_iter());
  
  JustificationTable {
    by_word,
    by_sub
  }
}

pub fn dictionary_base_score(dictionary: &Dictionary) -> f64 {
  let mut total: f64 = 0.0;
  
  for word in &dictionary.words {
    let score = distance(&word.spelling, &word.pronunciation) as f64;
    total = total + score * (word.frequency as f64);
  }
  
  total
}

pub fn score_dictionary_transformer<F: Fn(&Vec<Glyph>) -> Vec<Glyph>>(dictionary: &Dictionary, transformer: F) -> f64 {
  let mut total: f64 = 0.0;
  
  for word in &dictionary.words {
    let transformed = transformer(&word.spelling);
    let d = distance(&transformed, &word.pronunciation);
    let score = d as f64;
    total = total + score * (word.frequency as f64);
  }
  
  total
}

pub fn score_hl_slist(hl_slist: &HLSubstitutionList, dictionary: &Dictionary) -> f64 {
  score_dictionary_transformer(dictionary, |word| {
    apply_copied(word, &hl_slist)
  })
}

pub fn rate_edit(edit: &HLSubstitution, starting_hl_slist: &HLSubstitutionList, dictionary: &Dictionary) -> f64 {
  let mut hl_slist = starting_hl_slist.clone();
  hl_slist.substitutions.insert(0, edit.clone());
  score_hl_slist(&hl_slist, dictionary)
}

pub fn rate_top_edits<'a>(jt: &'a JustificationTable, max_to_rate: usize, starting_hl_slist: &HLSubstitutionList, dictionary: &Dictionary) -> Vec<(&'a HLSubstitution, f64)> {
  //use rayon::prelude::*;
  
  let mut best: Vec<(&'a HLSubstitution, &f64)> = jt.by_sub.iter().collect();
  best.sort_by_key(|(_, f)| FloatOrd(-*f));
  best.truncate(max_to_rate);
  
  let mut rated: Vec<(&'a HLSubstitution, f64)> = best.into_iter().map(|(edit, _)| {
    (edit, rate_edit(edit, starting_hl_slist, dictionary))
  }).collect();
  rated.sort_by_key(|(_, f)| FloatOrd(*f));
  
  rated
}

#[cfg(test)]
mod tests {
  use super::*;
  use Glyph::*;
  use crate::dictionary::DictionaryWord;
  use float_ord::FloatOrd;
  
  #[test]
  fn find_improving_edits_test_1() {
    let word = vec![E, I, G, H, T];
    let pronunciation = vec![Ei, T];
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    let found = find_improving_edits(&word, &pronunciation, &init_hl_slist);
    let found: Vec<HLSubstitution> = found.into_iter().map(|(s, _)| s).collect();
    assert!(found.contains(& HLSubstitution { key: vec![E, I], sub_start: 0, sub_end: 1, at_start: false, at_end: false, sub_content: vec![Ei] }));
  }
  
  #[test]
  fn find_improving_edits_test_2() {
    let word = vec![T, H, E];
    let pronunciation = vec![Dh, Eh];
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    let found = find_improving_edits(&word, &pronunciation, &init_hl_slist);
    let found: Vec<HLSubstitution> = found.into_iter().map(|(s, _)| s).collect();
    assert!(found.contains(& HLSubstitution { key: vec![T, H, E], sub_start: 0, sub_end: 3, at_start: true, at_end: true, sub_content: vec![Dh, Eh] }));
  }
  
  #[test]
  fn build_justification_table_test_1() {
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![E, I, G, H, T],
          pronunciation: vec![Ei, T],
          frequency: 1.0
        },
        DictionaryWord {
          spelling: vec![F, I, G, H, T],
          pronunciation: vec![F, I, T],
          frequency: 0.5
        }
      ]
    };
    let table = build_justification_table(&dictionary);
    
    assert_eq!((*table.by_sub.get(&HLSubstitution { key: vec![G, H], sub_start: 0, sub_end: 2, at_start: false, at_end: false, sub_content: vec![T] }).unwrap()*10.0) as u32, 13);
    
    for (word, word_entry) in table.by_word.iter() {
      println!("{:?}", word);
      for sub in &word_entry.substitutions {
        println!("  {:?}", sub);
      }
      println!("");
    }
    println!("");
    
    let mut best: Vec<(HLSubstitution, f64)> = table.by_sub.into_iter().collect();
    best.sort_by_key(|(_, f)| FloatOrd(-*f));
    
    for (sub, f) in best.iter().take(10) {
      println!("{:.3} {:?}", f, sub);
    }
  }
  
  #[test]
  fn distance_test_1() {
    let a = vec![E, I, G, H, T];
    let b = vec![Ei, T];
    let d = distance(&a, &b);
    println!("{:?} to {:?} = {}", a, b, d);
    assert_eq!(d, 4);
  }
  
  #[test]
  fn rate_edit_test_1() {
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![E, I, G, H, T],
          pronunciation: vec![Ei, T],
          frequency: 1.0
        },
        DictionaryWord {
          spelling: vec![F, I, G, H, T],
          pronunciation: vec![F, I, T],
          frequency: 0.5
        }
      ]
    };
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    
    let base_score = score_hl_slist(&init_hl_slist, &dictionary);
    println!("base_score = {}", base_score);
    
    let edit1 = HLSubstitution {
      key: vec![G, H, T],
      at_start: false,
      at_end: false,
      sub_start: 0,
      sub_end: 3,
      sub_content: vec![T],
    };
    
    let edit2 = HLSubstitution {
      key: vec![H, T],
      at_start: false,
      at_end: false,
      sub_start: 0,
      sub_end: 2,
      sub_content: vec![T],
    };
    
    let score1 = rate_edit(&edit1, &init_hl_slist, &dictionary);
    let score2 = rate_edit(&edit2, &init_hl_slist, &dictionary);
    
    println!("score1 = {}", score1);
    println!("score2 = {}", score2);
    
    assert!(score1 < score2);
  }
  
  #[test]
  fn rate_edit_test_2() {
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![E, I, G, H, T],
          pronunciation: vec![Ei, T],
          frequency: 1.0
        },
        DictionaryWord {
          spelling: vec![F, I, G, H, T],
          pronunciation: vec![F, I, T],
          frequency: 0.5
        }
      ]
    };
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![G, H, T],
          at_start: false,
          at_end: false,
          sub_start: 0,
          sub_end: 3,
          sub_content: vec![T],
        }
      ]
    };
    
    let base_score = score_hl_slist(&init_hl_slist, &dictionary);
    println!("base_score = {}", base_score);
    
    let edit1 = HLSubstitution {
      key: vec![E, I],
      at_start: false,
      at_end: false,
      sub_start: 0,
      sub_end: 2,
      sub_content: vec![Ei],
    };
    
    let edit2 = HLSubstitution {
      key: vec![G, H, T],
      at_start: false,
      at_end: false,
      sub_start: 0,
      sub_end: 3,
      sub_content: vec![T],
    };
    
    let score1 = rate_edit(&edit1, &init_hl_slist, &dictionary);
    let score2 = rate_edit(&edit2, &init_hl_slist, &dictionary);
    
    println!("score1 = {}", score1);
    println!("score2 = {}", score2);
    
    assert!(score1 < score2);
  }
  
  #[test]
  fn rate_top_edits_test_1() {
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![E, I, G, H, T],
          pronunciation: vec![Ei, T],
          frequency: 0.9
        },
        DictionaryWord {
          spelling: vec![T, H, R, E, E],
          pronunciation: vec![Th, R, Ee],
          frequency: 0.8
        },
      ]
    };
    let table = build_justification_table(&dictionary);
    
    let mut best: Vec<(HLSubstitution, f64)> = table.by_sub.iter().map(|(s,f)|(s.clone(), *f)).collect();
    best.sort_by_key(|(_, f)| FloatOrd(-*f));
    
    for (sub, f) in best.iter().take(10) {
      println!("{:.3} {:?}", f, sub);
    }
    println!("");
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![G, H, T],
          at_start: false,
          at_end: false,
          sub_start: 0,
          sub_end: 3,
          sub_content: vec![T],
        }
      ]
    };
    
    let top_edits = rate_top_edits(&table, 10, &init_hl_slist, &dictionary);
    for (sub, f) in top_edits.iter().take(10) {
      println!("{:.3} {:?}", f, sub);
    }
    println!("");
    
    assert_eq!(*top_edits[0].0, HLSubstitution { key: vec![E, I, G, H], sub_start: 0, sub_end: 4, at_start: false, at_end: false, sub_content: vec![Ei] });
  }
}

