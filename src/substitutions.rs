
use serde::{Serialize, Deserialize};

use crate::glyphs::Glyph;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Substitution {
  pub key: Vec<Glyph>,
  pub sub_start: usize,
  pub sub_end: usize,
  pub sub_content: Glyph
}

impl Substitution {
  pub fn render(&self) -> String {
    let lhs: Vec<String> = (0 .. self.key.len()).map(|i| {
      if self.sub_start <= i && i < self.sub_end {
        format!("{}'", self.key[i].name())
      }
      else {
        self.key[i].name()
      }
    }).collect();
    let rhs = self.sub_content.name();
    return format!("sub {} by {};", lhs.join(" "), rhs);
  }
  
  pub fn apply(&self, working: &mut Vec<Glyph>, pos: usize) -> bool {
    if pos < self.sub_start {
      false
    }
    else if pos - self.sub_start + self.key.len() > working.len() {
      false
    }
    else {
      let working_slice = &working[pos - self.sub_start .. pos - self.sub_start + self.key.len()];
      if working_slice == self.key {
        working.splice(pos .. pos + self.sub_end - self.sub_start, [self.sub_content]);
        true
      }
      else {
        false
      }
    }
  }
}

#[cfg(test)]
mod substitution_tests {
  use super::*;
  #[test]
  fn substitution_test_1() {
    use Glyph::*;
    let sub = Substitution {
      key: vec![Y, O, U],
      sub_start: 1,
      sub_end: 3,
      sub_content: X
    };
    let mut working = vec![Y, O, U];
    assert_eq!(sub.apply(&mut working, 0), false);
    assert_eq!(sub.apply(&mut working, 1), true);
    assert_eq!(working, vec![Y, X]);
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubstitutionListItem {
  Substitution(Substitution),
  Barrier
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstitutionList {
  pub substitutions: Vec<SubstitutionListItem>
}

impl SubstitutionList {
  pub fn apply_at_pos(&self, working: &mut Vec<Glyph>, pos: usize) {
    let mut i: usize = 0;
    let mut state = 0;
    
    loop {
      if i >= self.substitutions.len() {
        break;
      }
      
      if state == 0 {
        match &self.substitutions[i] {
          SubstitutionListItem::Barrier => (),
          SubstitutionListItem::Substitution(s) => {
            if s.apply(working, pos) {
              state = 1;
            }
          }
        }
      }
      else {
        match self.substitutions[i] {
          SubstitutionListItem::Barrier => {
            state = 0;
          },
          SubstitutionListItem::Substitution(_) => ()
        }
      }
      
      i = i + 1;
    }
  }
  
  // TODO: This is wrong. It has to run through one lookup first,
  // then on to the next lookup.
  pub fn apply_all_pos(&self, working: &mut Vec<Glyph>) {
    let mut i: usize = 0;
    while i < working.len() {
      self.apply_at_pos(working, i);
      i += 1;
    }
  }
  
  pub fn render(&self) -> String {
    let mut res = "".to_owned();
    let mut lookup_counter = 0;
    res.push_str(&format!("lookup l{} {{\n", lookup_counter));
    for sub in &self.substitutions {
      match sub {
        SubstitutionListItem::Barrier => {
          res.push_str(&format!("}} lookup l{};\n", lookup_counter));
          lookup_counter += 1;
          res.push_str(&format!("lookup l{} {{\n", lookup_counter));
        },
        SubstitutionListItem::Substitution(sub) => {
          res.push_str("  ");
          res.push_str(&sub.render());
          res.push_str("\n");
        }
      }
    }
    res.push_str(&format!("}} l{};\n", lookup_counter));
    res
  }
}

#[cfg(test)]
mod substitution_list_tests {
  use super::{SubstitutionList, Substitution, SubstitutionListItem, Glyph};

  #[test]
  fn substitution_list_test_1() {
    use Glyph::*;
    let mut substitution_list = SubstitutionList { substitutions: vec![] };
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Y, O, U],
      sub_start: 0,
      sub_end: 1,
      sub_content: Z
    }));
    substitution_list.substitutions.push(SubstitutionListItem::Barrier);
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Z, O, U],
      sub_start: 0,
      sub_end: 1,
      sub_content: X
    }));
    let mut working = vec![Y, O, U];
    substitution_list.apply_all_pos(&mut working);
    assert_eq!(working, vec![X, O, U]);
  }
  
  #[test]
  fn substitution_list_test_2() {
    use Glyph::*;
    let mut substitution_list = SubstitutionList { substitutions: vec![] };
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Y, O, U],
      sub_start: 0,
      sub_end: 1,
      sub_content: Z
    }));
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Z, O, U],
      sub_start: 0,
      sub_end: 1,
      sub_content: X
    }));
    let mut working = vec![Y, O, U];
    substitution_list.apply_all_pos(&mut working);
    assert_eq!(working, vec![Z, O, U]);
  }
  
  #[test]
  fn substitution_list_test_3() {
    use Glyph::*;
    let mut substitution_list = SubstitutionList { substitutions: vec![] };
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Y, O, U],
      sub_start: 0,
      sub_end: 1,
      sub_content: Z
    }));
    substitution_list.substitutions.push(SubstitutionListItem::Substitution(Substitution {
      key: vec![Z, O, U],
      sub_start: 2,
      sub_end: 3,
      sub_content: X
    }));
    let mut working = vec![Y, O, U];
    substitution_list.apply_all_pos(&mut working);
    assert_eq!(working, vec![Z, O, X]);
  }
}

