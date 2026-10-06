
use crate::substitutions2::*;
use crate::substitutions2 as s2;
use std::collections::HashSet;
use crate::glyphs::AugGlyph;

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "fea_grammar.pest"]
struct FEAParser;

#[cfg(test)]
mod fea_parser_tests {
  use super::*;
  use crate::glyphs::{Glyph, AugGlyph};
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }

  #[test]
  fn fea_parser_test_1() {
    assert_eq!(
      parse_fea_feature_body_1("lookup l0 { sub a by b; } l0;"),
      Ok(mid::Feature {
        lookups: vec![
          mid::Lookup {
            entries: vec![
              mid::LookupEntry::Sub(mid::Sub {
                lhs: vec![mid::LHSElement { glyph: "a".to_owned(), prime: false }],
                rhs: vec!["b".to_owned()]
              })
            ]
          }
        ]
      })
    );
  }
  
  #[test]
  fn fea_parser_test_2() {
    assert_eq!(
      parse_fea_feature_body_1("lookup l0 { ignore sub a' b'; } l0;"),
      Ok(mid::Feature {
        lookups: vec![
          mid::Lookup {
            entries: vec![
              mid::LookupEntry::Ignore(mid::Ignore {
                lhs: vec![mid::LHSElement { glyph: "a".to_owned(), prime: true }, mid::LHSElement { glyph: "b".to_owned(), prime: true }]
              })
            ]
          }
        ]
      })
    );
  }
  
  #[test]
  fn fea_parser_test_3() {
    use Glyph::*;
    assert_eq!(
      parse_fea_feature_body("lookup l0 { sub a b' c' d by e; } l0; lookup l1 { ignore sub a b' c' @lc; } l1;"),
      Ok(s2::SubstitutionList {
        lookups: vec![
          s2::Lookup {
            substitutions: vec![
              s2::Substitution {
                pre_key: vec![KeyElem::Glyph(AugGlyph::Real(A))],
                at_key: rr(&[B, C]),
                post_key: vec![KeyElem::Glyph(AugGlyph::Real(D))],
                sub_content: SubContent::Sub(rr(&[E]))
              },
            ],
          },
          s2::Lookup {
            substitutions: vec![
              s2::Substitution {
                pre_key: vec![KeyElem::Glyph(AugGlyph::Real(A))],
                at_key: rr(&[B, C]),
                post_key: vec![KeyElem::AnyLetter],
                sub_content: SubContent::Ignore
              }
            ]
          }
        ]
      })
    );
  }
  
  #[test]
  fn fea_parser_test_4() {
    use crate::glyphs::Glyph::*;
    let parsed = crate::fea_parsing::parse_fea_feature_body("
  lookup l0 {
    sub a' by b;
  } l0;
");
    assert_eq!(parsed, Ok(s2::SubstitutionList {
      lookups: vec![
        s2::Lookup {
          substitutions: vec![
            s2::Substitution {
              pre_key: vec![],
              at_key: rr(&[A]),
              post_key: vec![],
              sub_content: SubContent::Sub(rr(&[B]))
            }
          ]
        }
      ]
    }));
  }
}

fn key_elem_name(k: &KeyElem) -> String {
  match k {
    KeyElem::Glyph(g) => g.name(),
    KeyElem::AnyLetter => "@lc".to_owned(),
    KeyElem::Set(gs) => format!("[{}]", gs.iter().map(|g| g.name()).collect::<Vec<_>>().join(" "))
  }
}

pub fn render_fea_sub(s: &Substitution) -> String {
  let mut lhs = vec![];
  for g in &s.pre_key { lhs.push(key_elem_name(g)); }
  for g in &s.at_key { lhs.push(format!("{}'", g.name())); }
  for g in &s.post_key { lhs.push(key_elem_name(g)); }
  let lhs = lhs.join(" ");
  
  match &s.sub_content {
    SubContent::Ignore => {
      format!("ignore sub {};", lhs)
    },
    SubContent::Sub(sub_content) => {
      let rhs: Vec<String> = sub_content.iter().map(|g| g.name().to_owned()).collect();
      format!("sub {} by {};", lhs, rhs.join(" "))
    }
  }
}

// Renders the @lc class that KeyElem::AnyLetter refers to: every glyph in the
// font that AugGlyph::is_word_glyph accepts, including synthetic glyphs.
pub fn render_lc_class(font_glyph_names: &HashSet<String>) -> String {
  let mut members: Vec<AugGlyph> = font_glyph_names.iter()
    .filter_map(|name| AugGlyph::from_name(name))
    .filter(|g| g.is_word_glyph())
    .collect();
  members.sort();
  let names: Vec<String> = members.iter().map(|g| g.name()).collect();
  format!("@lc = [{}];", names.join(" "))
}

pub fn render_fea_feature_body(slist: &SubstitutionList) -> String {
  let mut res = "".to_owned();
  let mut lookup_counter = 0;
  for lookup in &slist.lookups {
    res.push_str(&format!("lookup l{} {{\n", lookup_counter));
    for sub in &lookup.substitutions {
      res.push_str("  ");
      res.push_str(&render_fea_sub(&sub));
      res.push_str("\n");
    }
    res.push_str(&format!("}} l{};\n", lookup_counter));
    lookup_counter += 1;
  }
  res
}

struct ParsedLHS {
  pre_key: Vec<KeyElem>,
  at_key: Vec<AugGlyph>,
  post_key: Vec<KeyElem>,
}

fn parse_key_elem(name: &str) -> Result<KeyElem, String> {
  if name == "@lc" {
    Ok(KeyElem::AnyLetter)
  }
  else {
    Ok(KeyElem::Glyph(AugGlyph::from_name(name).ok_or(format!("Can't parse glyph name {}", name))?))
  }
}

fn parse_lhs(lhs: &Vec<mid::LHSElement>) -> Result<ParsedLHS, String> {
  let mut pre_key: Vec<KeyElem> = vec![];
  let mut at_key: Vec<AugGlyph> = vec![];
  let mut post_key: Vec<KeyElem> = vec![];

  let mut state = 0;
  
  for elem in lhs {
    if state == 0 {
      if elem.prime { state = 1; }
    } else if state == 1 {
      if !elem.prime { state = 2; }
    }
    
    if state == 0 {
      pre_key.push(parse_key_elem(&elem.glyph)?);
    } else if state == 1 {
      at_key.push(AugGlyph::from_name(&elem.glyph).ok_or(format!("Can't parse glyph name {}", elem.glyph))?);
    } else {
      post_key.push(parse_key_elem(&elem.glyph)?);
    }
  }
  
  Ok(ParsedLHS { pre_key, at_key, post_key })
}

pub fn parse_fea_feature_body(text: &str) -> Result<SubstitutionList, String> {
  let f1 = parse_fea_feature_body_1(text)?;
  
  let mut lookups = vec![];
  
  for i in 0..f1.lookups.len() {
    let mut substitutions = vec![];
    
    let lookup = &f1.lookups[i];
    for entry in &lookup.entries {
      match entry {
        mid::LookupEntry::Ignore(ignore) => {
          let parsed_lhs = parse_lhs(&ignore.lhs)?;
          substitutions.push(s2::Substitution {
            pre_key: parsed_lhs.pre_key,
            at_key: parsed_lhs.at_key,
            post_key: parsed_lhs.post_key,
            sub_content: SubContent::Ignore
          })
        },
        mid::LookupEntry::Sub(sub) => {
          let parsed_lhs = parse_lhs(&sub.lhs)?;
          let mut parsed_rhs = vec![];
          for n in &sub.rhs {
            parsed_rhs.push(AugGlyph::from_name(n).ok_or(format!("Cannot parse glyph {}", n))?);
          }
          substitutions.push(s2::Substitution {
            pre_key: parsed_lhs.pre_key,
            at_key: parsed_lhs.at_key,
            post_key: parsed_lhs.post_key,
            sub_content: SubContent::Sub(parsed_rhs)
          })
        }
      }
    }
    
    lookups.push(s2::Lookup { substitutions });
  }
  
  Ok(SubstitutionList {
    lookups
  })
}

mod mid {
  #[derive(Debug, PartialEq, Eq)]
  pub struct Feature {
    pub lookups: Vec<Lookup>
  }

  #[derive(Debug, PartialEq, Eq)]
  pub enum LookupEntry {
    Sub(Sub),
    Ignore(Ignore)
  }

  #[derive(Debug, PartialEq, Eq)]
  pub struct Lookup {
    pub entries: Vec<LookupEntry>
  }

  #[derive(Debug, PartialEq, Eq)]
  pub struct Sub {
    pub lhs: Vec<LHSElement>,
    pub rhs: Vec<String>
  }

  #[derive(Debug, PartialEq, Eq)]
  pub struct Ignore {
    pub lhs: Vec<LHSElement>
  }

  #[derive(Debug, PartialEq, Eq)]
  pub struct LHSElement {
    pub glyph: String,
    pub prime: bool
  }
}

pub fn parse_fea_feature_body_1(text: &str) -> Result<mid::Feature, String> {
  let parse_result = FEAParser::parse(Rule::feature_body, text);
  match parse_result {
    Err(e) => {
      Err(format!("{}", e))
    }
    Ok(mut pr) => {
      let feature = pr.next().ok_or("No feature")?;
      
      let mut lookups = vec![];
      
      for lookup in feature.into_inner() {
        match lookup.as_rule() {
          Rule::name => (),
          Rule::lookup => {
            let mut entries = vec![];
            for sub in lookup.into_inner() {
              match sub.as_rule() {
                Rule::name => (),
                Rule::sub => {
                  let mut sub_lhs = vec![];
                  let mut sub_rhs = vec![];
                  
                  for child in sub.into_inner() {
                    match child.as_rule() {
                      Rule::lhs => {
                        let mut lhs = Box::new(child.into_inner());
                        loop {
                          let g = lhs.next();
                          if let Some(g) = g {
                            match g.as_rule() {
                              Rule::lhs_element => {
                                let mut name = None;
                                let mut prime = false;
                                for n in g.into_inner() {
                                  match n.as_rule() {
                                    Rule::name => {
                                      name = Some(n.as_str().to_owned());
                                    },
                                    Rule::prime => {
                                      prime = true;
                                    },
                                    other => Err(format!("Unrecognized rule (should be name or prime): {:?}", other))?
                                  }
                                }
                                if let Some(name) = name {
                                  sub_lhs.push(mid::LHSElement { glyph: name, prime });
                                }
                                else {
                                  Err("LHS elem has no name")?
                                }
                              },
                              Rule::lhs => {
                                *lhs = g.into_inner();
                              },
                              other => Err(format!("Unrecognized rule (should be lhs or lhs_element): {:?}", other))?
                            }
                          }
                          else { break; }
                        }
                      },
                      Rule::rhs => {
                        let rhs = child;
                        for g in rhs.into_inner() {
                          match g.as_rule() {
                            Rule::rhs_element => {
                              let mut name = None;
                              for n in g.into_inner() {
                                match n.as_rule() {
                                  Rule::name => {
                                    name = Some(n.as_str().to_owned());
                                  },
                                  other => Err(format!("Unrecognized rule (should be name): {:?}", other))?
                                }
                              }
                              if let Some(name) = name {
                                sub_rhs.push(name);
                              }
                              else {
                                Err("RHS elem has no name")?
                              }
                            },
                            other => Err(format!("Unrecognized rule (should be rhs_element): {:?}", other))?
                          }
                        }
                      },
                      other => Err(format!("Unrecognized rule (should be lhs or rhs): {:?}", other))?
                    }
                  }
                  
                  entries.push(mid::LookupEntry::Sub(mid::Sub {
                    lhs: sub_lhs,
                    rhs: sub_rhs
                  }))
                },
                Rule::ignore => {
                  let mut ignore_lhs = vec![];
                  
                  for child in sub.into_inner() {
                    match child.as_rule() {
                      Rule::lhs_element => {
                        let mut name = None;
                        let mut prime = false;
                        for n in child.into_inner() {
                          match n.as_rule() {
                            Rule::name => {
                              name = Some(n.as_str().to_owned());
                            },
                            Rule::prime => {
                              prime = true;
                            },
                            other => Err(format!("Unrecognized rule (should be name or prime): {:?}", other))?
                          }
                        }
                        if let Some(name) = name {
                          ignore_lhs.push(mid::LHSElement { glyph: name, prime });
                        }
                        else {
                          Err("LHS elem has no name")?
                        }
                      },
                      other => Err(format!("Unrecognized rule (should be lhs_element): {:?}", other))?
                    }
                  }
                  entries.push(mid::LookupEntry::Ignore(mid::Ignore {
                    lhs: ignore_lhs
                  }));
                },
                other => Err(format!("Unrecognized rule (should be name or sub): {:?}", other))?
              }
            }
            lookups.push(mid::Lookup { entries });
          },
          other => Err(format!("Unrecognized rule (should be name or lookup): {:?}", other))?
        }
      }
      
      Ok(mid::Feature { lookups })
    },
  }
}

