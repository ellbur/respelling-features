
use serde::{Serialize, Deserialize};

use crate::glyphs::AugGlyph;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Substitution {
  pub pre_key: Vec<KeyElem>,
  pub at_key: Vec<AugGlyph>,
  pub post_key: Vec<KeyElem>,
  pub sub_content: SubContent
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeyElem {
  Glyph(AugGlyph),
  AnyLetter,
  // Any of these glyphs (sorted).
  Set(Vec<AugGlyph>)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubContent {
  Ignore,
  Sub(Vec<AugGlyph>)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lookup {
  pub substitutions: Vec<Substitution>
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubstitutionList {
  pub lookups: Vec<Lookup>
}

pub fn matches(x: &AugGlyph, y: &KeyElem) -> bool {
  match y {
    KeyElem::Glyph(y) => x == y,
    KeyElem::AnyLetter => x.is_word_glyph(),
    KeyElem::Set(gs) => gs.contains(x)
  }
}

pub fn apply_sub_at_pos(working: &mut Vec<AugGlyph>, pos: usize, sub: &Substitution) -> bool {
  // x x x x x x x x
  //     ^           ^
  //     pos         working.len()
  //     
  //   a b b a
  //   ^-^-----^
  //    |   |
  //    |   key.len() - sub_start
  //    |
  //    sub_start
  
  assert!(sub.at_key.len() > 0);
  
  if pos < sub.pre_key.len() {
    return false;
  }
  else if pos + sub.at_key.len() + sub.post_key.len() > working.len() {
    return false;
  }
  else {
    let pre_match = (0..sub.pre_key.len()).all(|i| {
      matches(&working[i + pos - sub.pre_key.len()], &sub.pre_key[i])
    });
    
    if !pre_match { return false; }
    
    let at_match = (0..sub.at_key.len()).all(|i| {
      working[i + pos] == sub.at_key[i]
    });
    
    if !at_match { return false; }
    
    let post_match = (0..sub.post_key.len()).all(|i| {
      matches(&working[i + pos + sub.at_key.len()], &sub.post_key[i])
    });
    
    if !post_match { return false; }
    
    match &sub.sub_content {
      SubContent::Ignore => ( ),
      SubContent::Sub(sc) => {
        let working_s1 = pos;
        let working_s2 = pos + sub.at_key.len();
        working.splice(working_s1 .. working_s2, sc.clone());
      }
    };
    true
  }
}

#[cfg(test)]
mod one_tests {
  use super::*;
  use crate::glyphs::Glyph::*;
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }
  
  #[test]
  fn test_1() {
    let mut working = vec![r(A), r(B), r(C), r(D), r(E)];
    let sub = crate::fea_parsing::parse_fea_feature_body("lookup l0 { sub b c' d' by f; } l0;").unwrap().lookups.pop().unwrap().substitutions.pop().unwrap();
    assert_eq!(apply_sub_at_pos(&mut working, 0, &sub), false);
    assert_eq!(working, vec![r(A), r(B), r(C), r(D), r(E)]);
  }
  
  #[test]
  fn test_2() {
    let mut working = rr(&[A, B, C, D, E]);
    let sub = crate::fea_parsing::parse_fea_feature_body("lookup l0 { sub b c' d' by f; } l0;").unwrap().lookups.pop().unwrap().substitutions.pop().unwrap();
    assert_eq!(apply_sub_at_pos(&mut working, 2, &sub), true);
    assert_eq!(working, rr(&[A, B, F, E]));
  }
  
  #[test]
  fn test_3() {
    let mut working = rr(&[A, B, C, D, E]);
    let sub = crate::fea_parsing::parse_fea_feature_body("lookup l0 { sub g c' d' by f; } l0;").unwrap().lookups.pop().unwrap().substitutions.pop().unwrap();
    assert_eq!(apply_sub_at_pos(&mut working, 2, &sub), false);
    assert_eq!(working, rr(&[A, B, C, D, E]));
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  rule: Substitution,
  pos: usize,
  after: Vec<AugGlyph>
}

pub fn apply_all(working: &mut Vec<AugGlyph>, slist: &SubstitutionList) {
  for lookup in &slist.lookups {
    apply_lookup(working, lookup);
  }
}

// One lookup: at each position the first substitution that matches applies
// (or, for an ignore rule, blocks the rest), then the scan moves past it.
pub fn apply_lookup(working: &mut Vec<AugGlyph>, lookup: &Lookup) {
  let mut pos = 0;
  while pos < working.len() {
    'searching: {
      for sub in &lookup.substitutions {
        if apply_sub_at_pos(working, pos, sub) {
          pos += match &sub.sub_content {
            SubContent::Sub(s) => s.len(),
            SubContent::Ignore => sub.at_key.len()
          };
          break 'searching;
        }
      }
      pos += 1;
    }
  }
}

pub fn apply_all_with_new(working: &mut Vec<AugGlyph>, slist: &SubstitutionList, new_amount: usize) -> bool {
  let mut num_subs_in_prior_lookups: usize = 0;
  let mut any_new_matched = false;
  for lookup in &slist.lookups {
    if num_subs_in_prior_lookups >= new_amount && !any_new_matched {
      return false;
    }
    let mut pos = 0;
    while pos < working.len() {
      'searching: {
        for (sub_partial_i, sub) in lookup.substitutions.iter().enumerate() {
          let sub_full_i = sub_partial_i + num_subs_in_prior_lookups;
          let is_new = sub_full_i < new_amount;
          if apply_sub_at_pos(working, pos, sub) {
            if is_new { any_new_matched = true; }
            pos += match &sub.sub_content {
              SubContent::Sub(s) => s.len(),
              SubContent::Ignore => sub.at_key.len()
            };
            break 'searching;
          }
        }
        pos += 1;
      }
    }
    num_subs_in_prior_lookups += lookup.substitutions.len();
  }

  any_new_matched
}

#[cfg(test)]
mod two_tests {
  use super::*;
  use crate::glyphs::Glyph::*;
  use crate::fea_parsing::parse_fea_feature_body;
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }
  
  #[test]
  fn test_1() {
    let slist = parse_fea_feature_body("
").unwrap();
    let mut working = rr(&[A, B, C, D, E]);
    apply_all(&mut working, &slist);
    assert_eq!(working, rr(&[A, B, C, D, E]));
  }
  
  #[test]
  fn test_2() {
    let slist = parse_fea_feature_body("
      lookup l1 {
        sub b' by f;
      } l1;
      lookup l2 {
        sub f' by g;
      } l2;
").unwrap();
    let mut working = rr(&[A, B, C, D, E]);
    apply_all(&mut working, &slist);
    assert_eq!(working, rr(&[A, G, C, D, E]));
  }
  
  #[test]
  fn test_3() {
    let slist = parse_fea_feature_body("
      lookup l1 {
        sub a b' c' by f;
        sub a' f' by g;
      } l1;
      lookup l2 {
        sub f' d by h i;
        sub i d' by j;
      } l2;
").unwrap();
    let mut working = rr(&[A, B, C, D, E]);
    apply_all(&mut working, &slist);
    assert_eq!(working, rr(&[A, H, I, J, E]));
  }
  
  #[test]
  fn test_4() {
    let slist = parse_fea_feature_body("
      lookup l1 {
        sub a' by c d;
        sub d' by e;
      } l1;
").unwrap();
    let mut working = rr(&[A, B]);
    apply_all(&mut working, &slist);
    assert_eq!(working, rr(&[C, D, B]));
  }
}

