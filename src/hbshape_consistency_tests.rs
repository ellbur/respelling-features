
#[cfg(test)]
mod tests {
  use crate::fea_parsing::parse_fea_feature_body;
  use crate::glyphs::Glyph::*;
  use crate::glyphs::AugGlyph;
  use crate::hbshape::apply_using_hbshape;
  use crate::substitutions2::apply_all;
  use rand::{Rng, distributions::{Uniform, Bernoulli}, prelude::Distribution, thread_rng};
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }
  
  #[test]
  fn test_1() {
    let slist = parse_fea_feature_body("
      lookup l1 {
        sub a' by c d;
        sub d' by e;
      } l1;
").unwrap();
    let mut working = rr(&[A, B]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
  }
  
  #[test]
  fn found_by_random_test_1() {
    let slist = parse_fea_feature_body("
      lookup l0 {
        sub e' by c;
        ignore sub c' c';
        ignore sub d' e';
        sub a' b by b b;
      } l0;
").unwrap();
    let mut working = rr(&[D, E, E, E, A]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
  }
  
  #[test]
  fn found_by_random_test_2() {
    let slist = parse_fea_feature_body("
      lookup l0 {
        ignore sub a' a' @lc; sub a' a' by z;
      } l0;
").unwrap();
    let mut working = rr(&[A, A, A]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
    assert_eq!(by_hbshape, rr(&[A, A, A]));
  }
  
  #[test]
  fn found_by_random_test_3() {
    let slist = parse_fea_feature_body("
      lookup l0 {
        ignore sub a' a @lc; sub a' a' by z;
      } l0;
").unwrap();
    let mut working = rr(&[A, A, A]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
    assert_eq!(by_hbshape, rr(&[A, Z]));
  }
  
  #[test]
  fn found_by_random_test_4() {
    let slist = parse_fea_feature_body("
    lookup l0 {
      sub a' by syn0;
      sub b' a by syn1;
    } l0;
    lookup l1 {
      sub syn1' by a;
      sub syn0' by a;
    } l1;
").unwrap();
    let mut working = rr(&[B, A]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    println!("{:?}", by_hbshape);
    println!("{:?}", by_internal);
    assert_eq!(by_hbshape, by_internal);
  }
  
  #[test]
  fn ignore_order_test_1() {
    let slist = parse_fea_feature_body("
      lookup l0 {
        sub a' by b;
        ignore sub a';
      } l0;
").unwrap();
    let mut working = rr(&[A]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
  }
  
  #[test]
  fn ignore_order_test_2() {
    let slist = parse_fea_feature_body("
      lookup l0 {
        sub a' by c;
        ignore sub a' b';
      } l0;
").unwrap();
    let mut working = rr(&[A, B]);
    let init = working.clone();
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, by_internal);
  }
  
  fn make_random_slist<R: Rng>(rng: &mut R) -> crate::substitutions2::SubstitutionList {
    use crate::substitutions2::*;
    
    let sub_ignore_dist = Bernoulli::new(0.80).unwrap();
    let glyph_set = rr(&[A, B, C, D, E]);
    let glyph_i_dist = Uniform::new(0, glyph_set.len());
    
    let num_lookups = Uniform::new(1, 3).sample(rng);
    let mut lookups = vec![];
    for _ in 0 .. num_lookups {
      let mut substitutions = vec![];
      let num_substitutions = Uniform::new(1, 5).sample(rng);
      for _ in 0 .. num_substitutions {
        let num_in_key = Uniform::new(1, 4).sample(rng);
        let key: Vec<AugGlyph> = (0 .. num_in_key).map(|_| glyph_set[glyph_i_dist.sample(rng)]).collect();
        let num_in_target = Uniform::new(1, num_in_key+1).sample(rng);
        let sub_start = Uniform::new(0, num_in_key - num_in_target + 1).sample(rng);
        let sub_end = sub_start + num_in_target;
        let sub_content = if sub_ignore_dist.sample(rng) {
          let num_sub_content = if num_in_target == 1 {
            Uniform::new(1, 3).sample(rng)
          } else {
            1
          };
          let sc = (0 .. num_sub_content).map(|_| glyph_set[glyph_i_dist.sample(rng)]).collect();
          SubContent::Sub(sc)
        } else {
          SubContent::Ignore
        };
        
        substitutions.push(Substitution {
          pre_key: key[0 .. sub_start].iter().map(|g| KeyElem::Glyph(*g)).collect(),
          at_key: key[sub_start .. sub_end].to_owned(),
          post_key: key[sub_end ..].iter().map(|g| KeyElem::Glyph(*g)).collect(),
          sub_content
        });
      }
      lookups.push(Lookup { substitutions });
    }
    SubstitutionList { lookups }
  }
  
  #[test]
  fn there_back_test_1() {
    use crate::substitutions2::*;
    let slist = SubstitutionList {
      lookups: vec![
        Lookup {
          substitutions: vec![
            Substitution {
              pre_key: vec![],
              at_key: rr(&[E]),
              post_key: vec![KeyElem::Glyph(AugGlyph::Real(E)), KeyElem::Glyph(AugGlyph::Real(E))],
              sub_content: SubContent::Sub(rr(&[E]))
            }
          ]
        }
      ]
    };
    let slist_rendered = crate::fea_parsing::render_fea_feature_body(&slist);
    println!("{}", slist_rendered);
    let slist_back = crate::fea_parsing::parse_fea_feature_body(&slist_rendered).unwrap();
    assert_eq!(slist, slist_back);
  }
  
  #[test]
  fn there_back_test_2() {
    use crate::substitutions2::*;
    let slist = SubstitutionList { 
      lookups: vec![
        Lookup { 
          substitutions: vec![
            Substitution {
              pre_key: vec![KeyElem::Glyph(AugGlyph::Real(D))],
              at_key: rr(&[E, B]),
              post_key: vec![],
              sub_content: SubContent::Ignore 
            }
          ] 
        }
      ] 
    };
    let slist_rendered = crate::fea_parsing::render_fea_feature_body(&slist);
    println!("{}", slist_rendered);
    let slist_back = crate::fea_parsing::parse_fea_feature_body(&slist_rendered).unwrap();
    assert_eq!(slist, slist_back);
  }
  
  fn do_random_test<R: Rng>(rng: &mut R) {
    loop {
      let slist = make_random_slist(rng);
      
      let slist_rendered = crate::fea_parsing::render_fea_feature_body(&slist);
      let slist_back = crate::fea_parsing::parse_fea_feature_body(&slist_rendered).unwrap();
      assert_eq!(slist, slist_back);
      
      let glyph_set = rr(&[A, B, C, D, E]);
      let glyph_i_dist = Uniform::new(0, glyph_set.len());
      let input_size = Uniform::new(1, 6).sample(rng);
      let init: Vec<AugGlyph> = (0 .. input_size).map(|_| glyph_set[glyph_i_dist.sample(rng)]).collect();
      
      let mut working = init.clone();
      apply_all(&mut working, &slist);
      let by_internal = working.clone();
      if init == by_internal {
        continue;
      }
      
      let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
      
      if by_hbshape != by_internal {
        println!("init = {:?}", init);
        println!("slist =");
        println!("{}", slist_rendered);
        println!("by_hbshape = {:?}", by_hbshape);
        println!("by_internal = {:?}", by_internal);
      }
      
      assert_eq!(by_hbshape, by_internal);
      break;
    }
  }
  
  #[test]
  fn random_test_2() {
    use rand::SeedableRng;
    let mut rng = rand::rngs::SmallRng::seed_from_u64(0);
    do_random_test(&mut rng);
  }
  
  #[test]
  fn random_test_3() {
    let mut rng = thread_rng();
    for i in 0 .. 3 {
      println!("{}", i);
      do_random_test(&mut rng);
    }
  }
  
  #[test]
  fn hl_test_1() {
    use crate::high_level_substitutions::*;
    use crate::glyphs::*;
    use Glyph::*;
    use AugGlyph::*;
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![B, C],
          sub_start: 0,
          sub_end: 2,
          at_start: false,
          at_end: false,
          sub_content: vec![E, F]
        }
      ]
    };
    let slist = hl_to_ll(&hl_slist).unwrap();
    let init = vec![Real(A), Real(B), Real(C), Real(D)];
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    let mut working = init.clone();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, vec![Real(A), Real(E), Real(F), Real(D)]);
    assert_eq!(by_internal, vec![Real(A), Real(E), Real(F), Real(D)]);
  }
  
  #[test]
  fn hl_test_2() {
    use crate::high_level_substitutions::*;
    use crate::glyphs::*;
    use Glyph::*;
    use AugGlyph::*;
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![B, C],
          sub_start: 0,
          sub_end: 2,
          at_start: true,
          at_end: false,
          sub_content: vec![E, F]
        }
      ]
    };
    let slist = hl_to_ll(&hl_slist).unwrap();
    let init = vec![Real(A), Real(B), Real(C), Real(D)];
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    let mut working = init.clone();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, vec![Real(A), Real(B), Real(C), Real(D)]);
    assert_eq!(by_internal, vec![Real(A), Real(B), Real(C), Real(D)]);
  }
  
  #[test]
  fn hl_test_3() {
    use crate::high_level_substitutions::*;
    use crate::glyphs::*;
    use Glyph::*;
    use AugGlyph::*;
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution {
          key: vec![B, C],
          sub_start: 0,
          sub_end: 2,
          at_start: true,
          at_end: false,
          sub_content: vec![E, F]
        }
      ]
    };
    let slist = hl_to_ll(&hl_slist).unwrap();
    let init = vec![Real(B), Real(C), Real(D)];
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    let mut working = init.clone();
    apply_all(&mut working, &slist);
    let by_internal = working.clone();
    assert_eq!(by_hbshape, vec![Real(E), Real(F), Real(D)]);
    assert_eq!(by_internal, vec![Real(E), Real(F), Real(D)]);
  }
  
  // Applies an HL rule list three ways (HL simulation, low-level simulation,
  // harfbuzz) and checks that all three give `expected`.
  fn check_hl2(rules: &str, word: &str, expected: &str) {
    use crate::high_level_substitutions2::HLSubstitutionList;
    use crate::glyphs::{aug_decode, aug_encode};
    
    let hl_slist = HLSubstitutionList::decode(rules).unwrap();
    let slist = hl_slist.low_level();
    let init = aug_decode(word);
    
    let by_hl = hl_slist.apply_copied_always(&init);
    let mut by_internal = init.clone();
    apply_all(&mut by_internal, &slist);
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    
    println!("{}", crate::fea_parsing::render_fea_feature_body(&slist));
    assert_eq!(aug_encode(&by_hl), expected, "HL simulation");
    assert_eq!(aug_encode(&by_internal), expected, "low-level simulation");
    assert_eq!(aug_encode(&by_hbshape), expected, "harfbuzz");
  }
  
  #[test]
  fn boundary_after_token_test() {
    // ^ must not fire after a synthetic glyph: in "then", {1} follows {0}.
    let rules = "[th]→0→ϑ\n[e]→1→ʌ\n^[{1}]→2→ε";
    check_hl2(rules, "then", "ϑʌn");
    check_hl2(rules, "egg", "εgg");
  }
  
  #[test]
  fn boundary_before_token_test() {
    // $ must not fire before a synthetic glyph. The middle rule uses {0}, so it
    // starts a new lookup, and [i]$ joins it; otherwise [ng] and [i]$ would share
    // a lookup and [i]$ would see the letters n g instead of the token.
    let rules = "[ng]→0→ŋ\n[{0}]→1→ŋ\n[i]$→2→ɩ";
    check_hl2(rules, "ring", "riŋ");
    check_hl2(rules, "hiring", "hiriŋ");
    check_hl2(rules, "hi", "hɩ");
  }
  
  #[test]
  fn apostrophe_inside_word_test() {
    let rules = "[n]$→0→m\n^[t]→1→θ";
    check_hl2(rules, "don't", "don't");
    check_hl2(rules, "don", "dom");
    check_hl2(rules, "o'toole", "o'toole");
  }
  
  #[test]
  fn hyphen_between_words_test() {
    let rules = "[n]$→0→m\n^[o]→1→ɔ";
    check_hl2(rules, "on-off", "ɔm-ɔff");
  }
  
  // Applies half-rules (one per line) by simulation and by harfbuzz, and
  // checks that both give `expected`.
  fn check_half_rules(rules: &str, word: &str, expected: &str) {
    use crate::half_rules::{HalfRule, apply_all_copied, low_level};
    use crate::glyphs::{aug_decode, aug_encode};
    
    let rules: Vec<HalfRule> = rules.lines().map(|l| l.trim()).filter(|l| !l.is_empty())
      .map(|l| HalfRule::decode(l).unwrap()).collect();
    assert!(rules.iter().all(|r| r.is_valid()));
    let slist = low_level(&rules);
    let init = aug_decode(word);
    
    let by_sim = apply_all_copied(&rules, &init);
    let by_hbshape = apply_using_hbshape(&slist, &init).unwrap();
    
    println!("{}", crate::fea_parsing::render_fea_feature_body(&slist));
    assert_eq!(aug_encode(&by_sim), expected, "simulation");
    assert_eq!(aug_encode(&by_hbshape), expected, "harfbuzz");
  }
  
  #[test]
  fn half_rules_ligature_then_multiple_test() {
    check_half_rules("[th]→{0}\n[{0}]→ϑ", "the", "ϑe");
    check_half_rules("[a]t→{1}{2}\n[{1}]→e\n[{2}]→i", "at", "eit");
  }
  
  #[test]
  fn half_rules_contextual_multiple_test() {
    let rules = "o[x]→ks\n^[x]→z";
    check_half_rules(rules, "box", "boks");
    check_half_rules(rules, "xray", "zray");
    check_half_rules(rules, "axe", "axe");
  }
  
  #[test]
  fn half_rules_boundary_with_tokens_test() {
    let rules = "[ng]→{0}\n[i]{0}$→ɪ\n[{0}]→ŋ";
    check_half_rules(rules, "sing", "sɪŋ");
    check_half_rules(rules, "singer", "siŋer");
    check_half_rules(rules, "sing's", "siŋ's");
    check_half_rules(rules, "sing-song", "sɪŋ-soŋ");
  }
  
  #[test]
  fn half_rules_anti_anchors_test() {
    let rules = "~[s]→z\n[e]~→ɪ";
    check_half_rules(rules, "sass", "sazz");
    check_half_rules(rules, "the", "the");
    check_half_rules(rules, "then", "thɪn");
    check_half_rules(rules, "sees", "sɪɪz");
    check_half_rules(rules, "yes-sir", "yɪz-sir");
    check_half_rules(rules, "it's", "it'z");
    
    let rules = "[ng]→{0}\n~[{0}]→ŋ\n[{0}]→ng";
    check_half_rules(rules, "ngo", "ngo");
    check_half_rules(rules, "sing", "siŋ");
    check_half_rules(rules, "a-ngo", "a-ngo");
    
    let rules = "~t[o]~→u\n^[o]$→ɪ";
    check_half_rules(rules, "atom", "atum");
    check_half_rules(rules, "to", "to");
    check_half_rules(rules, "o", "ɪ");
    for r in ["~t[o]~→u", "~[s]→z", "[e]~→ɪ", "^[o]~→a", "~[o]$→a"] {
      assert_eq!(crate::half_rules::HalfRule::decode(r).unwrap().encode(), r);
    }
  }
  
  #[test]
  fn half_rules_glyph_sets_test() {
    let rules = "(ae)[x]→ks\n[s](ae)~→z";
    check_half_rules(rules, "ax", "aks");
    check_half_rules(rules, "ex", "eks");
    check_half_rules(rules, "ox", "ox");
    check_half_rules(rules, "sat", "zat");
    check_half_rules(rules, "set", "zet");
    check_half_rules(rules, "sa", "sa");
    check_half_rules(rules, "sax", "zaks");
    check_half_rules(rules, "sox", "sox");
    
    let rules = "[ng]→{0}\n({0}i)[k]→g\n[{0}]→ŋ";
    check_half_rules(rules, "ink", "ink");
    check_half_rules(rules, "ik", "ig");
    check_half_rules(rules, "ngk", "ŋg");
    
    let rules = "~(ae)t[o](mn)$→u";
    check_half_rules(rules, "baton", "batun");
    check_half_rules(rules, "tom", "tom");
    check_half_rules(rules, "etom", "etom");
    check_half_rules(rules, "betom", "betum");
    for r in ["(ae)[x]→ks", "[s](ae)~→z", "~(ae)t[o](mn)$→u", "(i{0})[k]→g"] {
      assert_eq!(crate::half_rules::HalfRule::decode(r).unwrap().encode(), r);
    }
  }
}
