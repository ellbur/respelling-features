
use serde::{Serialize, Deserialize};

use crate::glyphs::{Glyph, AugGlyph, MAX_SYN};
use crate::substitutions2 as s2;

use pest::Parser;
use pest_derive::Parser;
use pest::iterators::Pair;

#[derive(Parser)]
#[grammar = "hl_debug_grammar.pest"]
struct HLParser;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HLSubstitution {
  pub key: Vec<Glyph>,
  pub sub_start: usize,
  pub sub_end: usize,
  pub at_start: bool,
  pub at_end: bool,
  pub sub_content: Vec<Glyph>
}

impl HLSubstitution {
  pub fn from_debug_str(s: &str) -> Result<HLSubstitution, String> {
    let mut parse_result = HLParser::parse(Rule::hl_sub, s).unwrap();
    let top = parse_result.next().unwrap();
    
    let mut at_start = false;
    let mut at_end = false;
    
    let mut start_glyphs: Vec<Glyph> = vec![];
    let mut middle_glyphs: Vec<Glyph> = vec![];
    let mut end_glyphs: Vec<Glyph> = vec![];
    let mut rhs_glyphs: Vec<Glyph> = vec![];
    
    for part in top.into_inner() {
      match part.as_rule() {
        Rule::lhs => {
          for part in part.into_inner() {
            match part.as_rule() {
              Rule::at_start => {
                at_start = true;
              },
              Rule::lhs_body => {
                for part in part.into_inner() {
                  match part.as_rule() {
                    Rule::lhs_start => {
                      for part in part.into_inner() {
                        match part.as_rule() {
                          Rule::letter => { start_glyphs.push(extract_letter(part)); },
                          other => { panic!("{:?}", other) }
                        }
                      }
                    },
                    Rule::lhs_middle => {
                      for part in part.into_inner() {
                        match part.as_rule() {
                          Rule::letter => { middle_glyphs.push(extract_letter(part)); },
                          other => { panic!("{:?}", other) }
                        }
                      }
                    },
                    Rule::lhs_end => {
                      for part in part.into_inner() {
                        match part.as_rule() {
                          Rule::letter => { end_glyphs.push(extract_letter(part)); },
                          other => { panic!("{:?}", other) }
                        }
                      }
                    },
                    other => { panic!("{:?}", other) }
                  }
                }
              },
              Rule::at_end => {
                at_end = true;
              },
              other => { panic!("{:?}", other) }
            }
          }
        },
        Rule::rhs => {
          for part in part.into_inner() {
            match part.as_rule() {
              Rule::letter => { rhs_glyphs.push(extract_letter(part)); },
              other => panic!("{:?}", other)
            }
          }
        },
        other => { panic!("{:?}", other) }
      }
    }
    
    let sub_start = start_glyphs.len();
    let sub_end = start_glyphs.len() + middle_glyphs.len();
    
    let mut key: Vec<Glyph> = vec![];
    key.extend(start_glyphs);
    key.extend(middle_glyphs);
    key.extend(end_glyphs);
    
    let res = HLSubstitution {
      key,
      at_start,
      at_end,
      sub_start,
      sub_end,
      sub_content: rhs_glyphs
    };
    
    Ok(res)
  }
}

fn extract_letter(part: Pair<'_, Rule>) -> Glyph {
  use Glyph::*;
  match part.into_inner().next().unwrap().as_rule() {
    Rule::a      => A,
    Rule::b      => B     ,
    Rule::c      => C     ,
    Rule::d      => D     ,
    Rule::e      => E     ,
    Rule::f      => F     ,
    Rule::g      => G     ,
    Rule::h      => H     ,
    Rule::i      => I     ,
    Rule::j      => J     ,
    Rule::k      => K     ,
    Rule::l      => L     ,
    Rule::m      => M     ,
    Rule::n      => N     ,
    Rule::o      => O     ,
    Rule::p      => P     ,
    Rule::q      => Q     ,
    Rule::r      => R     ,
    Rule::s      => S     ,
    Rule::t      => T     ,
    Rule::u      => U     ,
    Rule::v      => V     ,
    Rule::w      => W     ,
    Rule::x      => X     ,
    Rule::y      => Y     ,
    Rule::z      => Z     ,
    Rule::ch     => Ch    ,
    Rule::th     => Th    ,
    Rule::sh     => Sh    ,
    Rule::jh     => Jh    ,
    Rule::eh     => Eh    ,
    Rule::ah     => Ah    ,
    Rule::oi     => Oi    ,
    Rule::ow     => Ow    ,
    Rule::aw     => Aw    ,
    Rule::eu     => Eu    ,
    Rule::uh     => Uh    ,
    Rule::ee     => Ee    ,
    Rule::ei     => Ei    ,
    Rule::yu     => Yu    ,
    Rule::dh     => Dh    ,
    Rule::ng     => Ng    ,
    Rule::ae     => Ae    ,
    Rule::ih     => Ih    ,
    Rule::schwa  => Schwa ,
    Rule::hyphen => Hyphen,
    Rule::er     => Er    ,
    Rule::apos   => Apos  ,
    other => panic!("{:?}", other)
  }
}
  
impl std::fmt::Debug for HLSubstitution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    if self.at_start { f.write_str("^")? };
    for i in 0 .. self.key.len() {
      if self.sub_start == i { f.write_str("[")?; }
      f.write_str(&self.key[i].char())?;
      if self.sub_end == i + 1 { f.write_str("]")?; }
    }
    if self.at_end { f.write_str("$")? };
    f.write_str("→")?;
    for g in &self.sub_content {
      f.write_str(&g.char())?;
    }
    Ok(())
  }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HLSubstitutionList {
  pub substitutions: Vec<HLSubstitution>
}

impl HLSubstitutionList {
  pub fn from_debug_str(s: &str) -> Result<HLSubstitutionList, String> {
    if s.trim().len() == 0 {
      Ok(HLSubstitutionList { substitutions: vec![] })
    }
    else if s.contains(",") {
      Ok(HLSubstitutionList {
        substitutions: s.split(",").map(|t| HLSubstitution::from_debug_str(t.trim())).collect::<Result<Vec<_>, _>>()?
      })
    }
    else {
      let mut substitutions = s.split("\n").map(|t| HLSubstitution::from_debug_str(t.trim())).collect::<Result<Vec<_>, _>>()?;
      substitutions.reverse();
      Ok(HLSubstitutionList {
        substitutions
      })
    }
  }
}

impl std::fmt::Debug for HLSubstitutionList {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    let n = self.substitutions.len();
    for (i, s) in self.substitutions.iter().enumerate() {
      f.write_str(&format!("{:?}", s))?;
      if i < n {
        f.write_str(", ")?;
      }
    }
    Ok(())
  }
}

pub fn hl_to_ll(hslist: &HLSubstitutionList) -> Result<s2::SubstitutionList, String> {
  use std::collections::HashMap;
  
  let mut lookups: Vec<s2::Lookup> = vec![];
  let mut working_l0 = vec![];
  
  let mut synthetic_count: u32 = 0;
  let mut synthetic_table: HashMap<Vec<Glyph>, u32> = HashMap::new();
  
  for sub in &hslist.substitutions {
    let sub_content_k = match synthetic_table.get(&sub.sub_content) {
      Some(k) => *k,
      None => {
        if (synthetic_count as usize) >= MAX_SYN {
          return Err(format!("Exceeded max number of syn glyphs {}", MAX_SYN));
        }
        
        let k = synthetic_count;
        synthetic_count += 1;
        synthetic_table.insert(sub.sub_content.clone(), k);
        k
      }
    };
    let sub_content = vec![AugGlyph::Synthetic(sub_content_k)];
    
    let key: Vec<AugGlyph> = sub.key.iter().map(|g| AugGlyph::Real(g.clone())).collect();
    let sub_start = sub.sub_start;
    let sub_end = sub.sub_end;
    
    let pre_key: Vec<s2::KeyElem> = key[..sub_start].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    let at_key: Vec<AugGlyph> = key[sub_start..sub_end].to_owned();
    let post_key: Vec<s2::KeyElem> = key[sub_end..].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    
    if sub.at_start {
      let mut start_pre_key = vec![s2::KeyElem::AnyLetter];
      start_pre_key.extend(pre_key.clone());
      working_l0.push(s2::Substitution {
        pre_key: start_pre_key,
        at_key: at_key.clone(),
        post_key: post_key.clone(),
        sub_content: s2::SubContent::Ignore
      });
    }
    
    if sub.at_end {
      let mut end_post_key = post_key.clone();
      end_post_key.push(s2::KeyElem::AnyLetter);
      working_l0.push(s2::Substitution {
        pre_key: pre_key.clone(),
        at_key: at_key.clone(),
        post_key: end_post_key,
        sub_content: s2::SubContent::Ignore
      });
    }
    
    working_l0.push(s2::Substitution {
      pre_key,
      at_key,
      post_key,
      sub_content: s2::SubContent::Sub(sub_content)
    });

    if sub.at_start || sub.at_end {
      lookups.push(s2::Lookup { substitutions: working_l0 });
      working_l0 = vec![];
    }
  }
  
  if !working_l0.is_empty() {
    lookups.push(s2::Lookup { substitutions: working_l0 } );
  }
  
  let mut l1 = vec![];
  
  for (gs, k) in synthetic_table.into_iter() {
    l1.push(s2::Substitution {
      pre_key: vec![],
      at_key: vec![AugGlyph::Synthetic(k)],
      post_key: vec![],
      sub_content: s2::SubContent::Sub(gs.into_iter().map(|g| AugGlyph::Real(g)).collect())
    });
  }
  
  lookups.push(s2::Lookup { substitutions: l1 });
  
  Ok(s2::SubstitutionList { lookups })
}

pub fn hl_to_ll_with_new(hslist: &HLSubstitutionList, new_amount: usize) -> Result<(s2::SubstitutionList, usize), String> {
  use std::collections::HashMap;
  
  let mut lookups: Vec<s2::Lookup> = vec![];
  let mut working_l0 = vec![];
  
  let mut synthetic_count: u32 = 0;
  let mut synthetic_table: HashMap<Vec<Glyph>, u32> = HashMap::new();
  
  let mut output_new_amount: usize = 0;
  
  for (sub_i, sub) in hslist.substitutions.iter().enumerate() {
    let is_new = sub_i < new_amount;
    let mut num_output_subs: usize = 0;
    
    let sub_content_k = match synthetic_table.get(&sub.sub_content) {
      Some(k) => *k,
      None => {
        if (synthetic_count as usize) >= MAX_SYN {
          return Err(format!("Exceeded max number of syn glyphs {}", MAX_SYN));
        }
        
        let k = synthetic_count;
        synthetic_count += 1;
        synthetic_table.insert(sub.sub_content.clone(), k);
        k
      }
    };
    let sub_content = vec![AugGlyph::Synthetic(sub_content_k)];
    
    let key: Vec<AugGlyph> = sub.key.iter().map(|g| AugGlyph::Real(g.clone())).collect();
    let sub_start = sub.sub_start;
    let sub_end = sub.sub_end;
    
    let pre_key: Vec<s2::KeyElem> = key[..sub_start].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    let at_key: Vec<AugGlyph> = key[sub_start..sub_end].to_owned();
    let post_key: Vec<s2::KeyElem> = key[sub_end..].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    
    if sub.at_start {
      let mut start_pre_key = vec![s2::KeyElem::AnyLetter];
      start_pre_key.extend(pre_key.clone());
      num_output_subs += 1;
      working_l0.push(s2::Substitution {
        pre_key: start_pre_key,
        at_key: at_key.clone(),
        post_key: post_key.clone(),
        sub_content: s2::SubContent::Ignore
      });
    }
    
    if sub.at_end {
      let mut end_post_key = post_key.clone();
      end_post_key.push(s2::KeyElem::AnyLetter);
      num_output_subs += 1;
      working_l0.push(s2::Substitution {
        pre_key: pre_key.clone(),
        at_key: at_key.clone(),
        post_key: end_post_key,
        sub_content: s2::SubContent::Ignore
      });
    }
    
    num_output_subs += 1;
    working_l0.push(s2::Substitution {
      pre_key,
      at_key,
      post_key,
      sub_content: s2::SubContent::Sub(sub_content)
    });

    if sub.at_start || sub.at_end {
      lookups.push(s2::Lookup { substitutions: working_l0 });
      working_l0 = vec![];
    }
    
    if is_new {
      output_new_amount += num_output_subs;
    }
  }
  
  if !working_l0.is_empty() {
    lookups.push(s2::Lookup { substitutions: working_l0 } );
  }
  
  let mut l1 = vec![];
  
  for (gs, k) in synthetic_table.into_iter() {
    l1.push(s2::Substitution {
      pre_key: vec![],
      at_key: vec![AugGlyph::Synthetic(k)],
      post_key: vec![],
      sub_content: s2::SubContent::Sub(gs.into_iter().map(|g| AugGlyph::Real(g)).collect())
    });
  }
  
  lookups.push(s2::Lookup { substitutions: l1 });
  
  Ok((s2::SubstitutionList { lookups }, output_new_amount))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::glyphs::Glyph::*;
  
  #[test]
  fn test_1() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![A, B],
          sub_start: 0,
          sub_end: 2,
          at_start: false,
          at_end: false,
          sub_content: vec![C, D]
        }
      ]
    };
    let slist = hl_to_ll(&hl_slist).unwrap();
    assert_eq!(slist, s2::SubstitutionList {
      lookups: vec![
        s2::Lookup {
          substitutions: vec![
            s2::Substitution {
              pre_key: vec![],
              at_key: vec![AugGlyph::Real(A), AugGlyph::Real(B)],
              post_key: vec![],
              sub_content: s2::SubContent::Sub(vec![AugGlyph::Synthetic(0)])
            }
          ]
        },
        s2::Lookup {
          substitutions: vec![
            s2::Substitution {
              pre_key: vec![],
              at_key: vec![AugGlyph::Synthetic(0)],
              post_key: vec![],
              sub_content: s2::SubContent::Sub(vec![AugGlyph::Real(C), AugGlyph::Real(D)])
            }
          ]
        }
      ]
    });
  }
  
  #[test]
  fn test_2() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![A, B],
          sub_start: 0,
          sub_end: 2,
          at_start: true,
          at_end: true,
          sub_content: vec![C, D]
        }
      ]
    };
    let slist = hl_to_ll(&hl_slist).unwrap();
    assert_eq!(slist, s2::SubstitutionList {
      lookups: vec![
        s2::Lookup {
          substitutions: vec![
            s2::Substitution {
              pre_key: vec![s2::KeyElem::AnyLetter],
              at_key: vec![AugGlyph::Real(A), AugGlyph::Real(B)],
              post_key: vec![],
              sub_content: s2::SubContent::Ignore
            },
            s2::Substitution {
              pre_key: vec![],
              at_key: vec![AugGlyph::Real(A), AugGlyph::Real(B)],
              post_key: vec![s2::KeyElem::AnyLetter],
              sub_content: s2::SubContent::Ignore
            },
            s2::Substitution {
              pre_key: vec![],
              at_key: vec![AugGlyph::Real(A), AugGlyph::Real(B)],
              post_key: vec![],
              sub_content: s2::SubContent::Sub(vec![AugGlyph::Synthetic(0)])
            }
          ]
        },
        s2::Lookup {
          substitutions: vec![
            s2::Substitution {
              pre_key: vec![],
              at_key: vec![AugGlyph::Synthetic(0)],
              post_key: vec![],
              sub_content: s2::SubContent::Sub(vec![AugGlyph::Real(C), AugGlyph::Real(D)])
            }
          ]
        }
      ]
    });
  }
}

#[cfg(test)]
mod grammar_tests {
  use super::*;
  
  #[test]
  fn test_hl_debug_grammar_1() {
    use Glyph::*;
    assert_eq!(HLSubstitution::from_debug_str("^[to]$→tu").unwrap(), HLSubstitution {
      at_start: true,
      at_end: true,
      key: vec![T, O],
      sub_start: 0,
      sub_end: 2,
      sub_content: vec![T, U]
    });
  }
  
  #[test]
  fn test_hl_debug_grammar_2() {
    use Glyph::*;
    assert_eq!(HLSubstitutionList::from_debug_str("^[to]$→tu, ^[th]e→ϑ").unwrap(), HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          at_start: true,
          at_end: true,
          key: vec![T, O],
          sub_start: 0,
          sub_end: 2,
          sub_content: vec![T, U]
        },
        HLSubstitution {
          at_start: true,
          at_end: false,
          key: vec![T, H, E],
          sub_start: 0,
          sub_end: 2,
          sub_content: vec![Dh]
        },
      ]
    });
  }
}

