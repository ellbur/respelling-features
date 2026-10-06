// The first pass (astarlike2::IterativeSystem) expressed as a genastarlike
// EditSystem, so the same search can drive the later passes.
//
// Each search finds one new rule. It sees the dictionary as transformed by the
// rules found so far: spellings with every anterior applied, and
// pronunciations with every posterior de-applied, which is where candidate
// rules come from. Candidates are scored by the real distance, after applying
// the posteriors forward.

use rayon::prelude::*;

use crate::astarlike2::{self, AugDictionary};
use crate::dictionary::Dictionary;
use crate::genastarlike::{self, EditSystem, EstimationSystem, Outcome, SubWithImprovement, Table};
use crate::pargenastarlike::{self, ParParams};
use crate::glyphs::{AugGlyph, aug_encode};
use crate::high_level_substitutions2::{Anterior, HLSubstitution, HLSubstitutionList, Posterior};

// What each token finally turns into once every posterior has been applied.
// A posterior's content only refers to tokens of earlier rules, whose
// posteriors run after it, so applying all the posteriors to a word is the
// same as replacing each token with its expansion.
struct Expansions {
  by_mid: Vec<Option<Vec<AugGlyph>>>
}

impl Expansions {
  fn new(rules: &HLSubstitutionList) -> Expansions {
    let mut res = Expansions { by_mid: vec![] };
    for sub in &rules.substitutions {
      let mut expansion = vec![];
      res.expand_into(&sub.posterior.content, &mut expansion);
      let mid = sub.mid as usize;
      if res.by_mid.len() <= mid {
        res.by_mid.resize(mid + 1, None);
      }
      res.by_mid[mid] = Some(expansion);
    }
    res
  }
  
  fn get(&self, g: &AugGlyph) -> Option<&Vec<AugGlyph>> {
    match g {
      AugGlyph::Synthetic(m) => self.by_mid.get(*m as usize).and_then(|e| e.as_ref()),
      AugGlyph::Real(_) => None
    }
  }
  
  fn expand_into(&self, word: &[AugGlyph], out: &mut Vec<AugGlyph>) {
    for g in word {
      match self.get(g) {
        Some(expansion) => out.extend_from_slice(expansion),
        None => out.push(*g)
      }
    }
  }
  
  // Like expand_into, but `new_mid` (a candidate rule's token) expands to
  // `new_expansion`.
  fn expand_with_new_into(&self, word: &[AugGlyph], new_mid: u32, new_expansion: &[AugGlyph], out: &mut Vec<AugGlyph>) {
    for g in word {
      if *g == AugGlyph::Synthetic(new_mid) {
        out.extend_from_slice(new_expansion);
      }
      else {
        match self.get(g) {
          Some(expansion) => out.extend_from_slice(expansion),
          None => out.push(*g)
        }
      }
    }
  }
}

// Anterior::apply, writing the result into `out` instead of editing a copy of
// `word` in place. The output so far is the already-rewritten prefix that
// Anterior::apply's lookbehind would see.
fn apply_anterior_into(anterior: &Anterior, mid: u32, word: &[AugGlyph], out: &mut Vec<AugGlyph>) -> bool {
  let (pre, at, post) = (&anterior.pre_key, &anterior.at_key, &anterior.post_key);
  out.clear();
  let mut i = 0;
  let mut any_mod = false;
  while i < word.len() {
    let pos = out.len();
    let matches =
         pos >= pre.len()
      && i + at.len() + post.len() <= word.len()
      && !(anterior.at_start && pos > pre.len() && out[pos - pre.len() - 1].is_word_glyph())
      && !(anterior.at_end && i + at.len() + post.len() < word.len() && word[i + at.len() + post.len()].is_word_glyph())
      && !(anterior.not_at_start && !(pos > pre.len() && out[pos - pre.len() - 1].is_word_glyph()))
      && !(anterior.not_at_end && !(i + at.len() + post.len() < word.len() && word[i + at.len() + post.len()].is_word_glyph()))
      && out[pos - pre.len() ..] == pre[..]
      && word[i .. i + at.len()] == at[..]
      && word[i + at.len() .. i + at.len() + post.len()] == post[..];
    if matches {
      out.push(AugGlyph::Synthetic(mid));
      i += at.len();
      any_mod = true;
    }
    else {
      out.push(word[i]);
      i += 1;
    }
  }
  any_mod
}

thread_local! {
  // Scratch space for new_distance: the anterior's output, the candidate's
  // expanded content, and the final output.
  static SCRATCH: std::cell::RefCell<(Vec<AugGlyph>, Vec<AugGlyph>, Vec<AugGlyph>)> = std::cell::RefCell::new((vec![], vec![], vec![]));
}

pub struct FirstPassEditSystem<'d> {
  dictionary: &'d AugDictionary,
  mid: u32,
  // Whether candidates may be anchored to the start (^) or end ($) of a word.
  pub allow_anchors: bool,
  // Whether candidates may be anti-anchored (~): kept from the start or end
  // of a word by requiring a word glyph there.
  pub allow_anti_anchors: bool,
  // Whether candidates may have lookbehind or lookahead context.
  pub allow_context: bool,
  expansions: Expansions,
  current_distances: Vec<u32>
}

impl<'d> FirstPassEditSystem<'d> {
  pub fn new(dictionary: &'d AugDictionary, rules: &'d HLSubstitutionList, mid: u32) -> FirstPassEditSystem<'d> {
    let expansions = Expansions::new(rules);
    let current_distances = dictionary.words.iter().map(|w| {
      let mut output = vec![];
      expansions.expand_into(&w.transformed_spelling, &mut output);
      astarlike2::distance(&output, &w.base_pronunciation)
    }).collect();
    
    FirstPassEditSystem { dictionary, mid, allow_anchors: true, allow_anti_anchors: false, allow_context: true, expansions, current_distances }
  }
}

impl<'d> EditSystem<HLSubstitution> for FirstPassEditSystem<'d> {
  // The same candidates, in the same order, as astarlike2::find_improving_edits,
  // but each anterior is applied once rather than once per content, and the
  // result is scored by expanding tokens rather than applying every posterior.
  fn find_improving_edits(&self, word: usize) -> Vec<SubWithImprovement<HLSubstitution>> {
    let w = &self.dictionary.words[word];
    let spelling = &w.transformed_spelling;
    let pronunciation = &w.back_transformed_pronunciation;
    let base_distance = self.current_distances[word];
    let mid = self.mid;
    
    // content_expansions[sc1][sc2 - sc1 - 1] is pronunciation[sc1 .. sc2] expanded.
    let content_expansions: Vec<Vec<Vec<AugGlyph>>> = (0 .. pronunciation.len()).map(|sc1| {
      (sc1 + 1 ..= pronunciation.len()).map(|sc2| {
        let mut e = vec![];
        self.expansions.expand_into(&pronunciation[sc1 .. sc2], &mut e);
        e
      }).collect()
    }).collect();
    
    (0 .. spelling.len()).into_par_iter().map(|k1| {
      let mut res = vec![];
      let mut output: Vec<AugGlyph> = vec![];
      
      for k2 in (k1 + 1) .. (spelling.len() + 1) {
        let key_size = k2 - k1;
        // A key at a word boundary can be anchored there (^/$); one inside a
        // word can be anti-anchored (~). "at_start" below means whichever of
        // the two applies.
        let start_is_boundary = k1 == 0 || !spelling[k1 - 1].is_word_glyph();
        let end_is_boundary = k2 == spelling.len() || !spelling[k2].is_word_glyph();
        let can_be_at_start = if start_is_boundary { self.allow_anchors } else { self.allow_anti_anchors };
        let can_be_at_end = if end_is_boundary { self.allow_anchors } else { self.allow_anti_anchors };
        let whole_key = &spelling[k1 .. k2];
        
        for s1 in 0 .. key_size {
          for s2 in (s1 + 1) .. (key_size + 1) {
            if !self.allow_context && (s1 > 0 || s2 < key_size) {
              continue;
            }
            let anterior_for = |at_start: bool, at_end: bool| Anterior {
              at_start: at_start && start_is_boundary,
              at_end: at_end && end_is_boundary,
              not_at_start: at_start && !start_is_boundary,
              not_at_end: at_end && !end_is_boundary,
              pre_key: whole_key[.. s1].to_vec(),
              at_key: whole_key[s1 .. s2].to_vec(),
              post_key: whole_key[s2 ..].to_vec()
            };
            
            // The spelling after each (at_start, at_end) variant of this
            // anterior, or None if it doesn't match.
            let applied = |at_start: bool, at_end: bool| -> Option<Vec<AugGlyph>> {
              let mut word = spelling.clone();
              if anterior_for(at_start, at_end).apply(&mut word, mid) { Some(word) } else { None }
            };
            let applied_tt = if can_be_at_start && can_be_at_end { applied(true, true) } else { None };
            let applied_tf = if can_be_at_start { applied(true, false) } else { None };
            let applied_ft = if can_be_at_end { applied(false, true) } else { None };
            let applied_ff = applied(false, false);
            
            for sc1 in 0 .. pronunciation.len() {
              for sc2 in (sc1 + 1) .. (pronunciation.len() + 1) {
                let content_expansion = &content_expansions[sc1][sc2 - sc1 - 1];
                
                for &at_start in if can_be_at_start {[true, false].iter()} else {[false].iter()} {
                  for &at_end in if can_be_at_end {[true, false].iter()} else {[false].iter()} {
                    let applied = match (at_start, at_end) {
                      (true, true) => &applied_tt,
                      (true, false) => &applied_tf,
                      (false, true) => &applied_ft,
                      (false, false) => &applied_ff
                    };
                    if let Some(applied) = applied {
                      output.clear();
                      self.expansions.expand_with_new_into(applied, mid, content_expansion, &mut output);
                      let new_distance = astarlike2::distance(&output, &w.base_pronunciation);
                      if new_distance < base_distance {
                        let sub = HLSubstitution {
                          anterior: anterior_for(at_start, at_end),
                          mid,
                          posterior: Posterior {
                            content: pronunciation[sc1 .. sc2].to_vec()
                          }
                        };
                        res.push(SubWithImprovement {
                          size_cost: astarlike2::edit_size_cost(&sub),
                          sub,
                          improvement: base_distance - new_distance
                        });
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
    }).flatten().collect()
  }
  
  fn distance(&self, word: usize) -> u32 {
    self.current_distances[word]
  }
  
  fn new_distance(&self, new_rule: &HLSubstitution, word: usize) -> Option<u32> {
    let w = &self.dictionary.words[word];
    SCRATCH.with(|scratch| {
      let (applied, new_expansion, output) = &mut *scratch.borrow_mut();
      if !apply_anterior_into(&new_rule.anterior, new_rule.mid, &w.transformed_spelling, applied) {
        return None;
      }
      new_expansion.clear();
      self.expansions.expand_into(&new_rule.posterior.content, new_expansion);
      output.clear();
      self.expansions.expand_with_new_into(applied, new_rule.mid, new_expansion, output);
      Some(astarlike2::distance(output, &w.base_pronunciation))
    })
  }
  
  fn describe_word(&self, word: usize) -> String {
    aug_encode(&self.dictionary.words[word].transformed_spelling)
  }
}

pub struct FirstPass {
  pub dictionary: AugDictionary,
  pub rules: HLSubstitutionList,
  // Whether rules may be anchored to the start (^) or end ($) of a word.
  pub allow_anchors: bool,
  // Whether rules may be anti-anchored (~).
  pub allow_anti_anchors: bool,
  // Whether rules may have lookbehind or lookahead context.
  pub allow_context: bool
}

impl FirstPass {
  pub fn setup(dictionary: &Dictionary, init_rules: HLSubstitutionList) -> FirstPass {
    let astarlike2::IterativeSystem { dictionary, rules } = astarlike2::IterativeSystem::setup(dictionary, init_rules);
    FirstPass { dictionary, rules, allow_anchors: true, allow_anti_anchors: false, allow_context: true }
  }
  
  // A first pass that has already found `rules`, to continue from there.
  pub fn resume(dictionary: &Dictionary, rules: &HLSubstitutionList) -> FirstPass {
    let mut pass = FirstPass::setup(dictionary, HLSubstitutionList { substitutions: vec![] });
    for sub in &rules.substitutions {
      pass.add_rule(sub);
    }
    pass
  }
  
  fn add_rule(&mut self, sub: &HLSubstitution) {
    self.rules.substitutions.push(sub.clone());
    for w in self.dictionary.words.iter_mut() {
      sub.apply_anterior(&mut w.transformed_spelling);
      sub.deapply_posterior(&mut w.back_transformed_pronunciation);
    }
  }

  pub fn find_next_rule<
      T: Table<Estimate, Estimator, HLSubstitution> + Send + Sync,
      Estimate: Clone + Send + Sync,
      Estimator: Clone + Send + Sync,
      EstSys: EstimationSystem<T, Estimate, Estimator, HLSubstitution> + Send + Sync
    >(&mut self, est_sys: &EstSys, debug: bool) -> Outcome<HLSubstitution>
  {
    self.find_next_rule_with(est_sys, None, debug)
  }
  
  // find_next_rule using pargenastarlike's search on several threads.
  pub fn find_next_rule_parallel<
      T: Table<Estimate, Estimator, HLSubstitution> + Send + Sync,
      Estimate: Clone + Send + Sync,
      Estimator: Clone + Send + Sync,
      EstSys: EstimationSystem<T, Estimate, Estimator, HLSubstitution> + Send + Sync
    >(&mut self, est_sys: &EstSys, params: ParParams) -> Outcome<HLSubstitution>
  {
    self.find_next_rule_with(est_sys, Some(params), false)
  }
  
  fn find_next_rule_with<
      T: Table<Estimate, Estimator, HLSubstitution> + Send + Sync,
      Estimate: Clone + Send + Sync,
      Estimator: Clone + Send + Sync,
      EstSys: EstimationSystem<T, Estimate, Estimator, HLSubstitution> + Send + Sync
    >(&mut self, est_sys: &EstSys, parallel: Option<ParParams>, debug: bool) -> Outcome<HLSubstitution>
  {
    let outcome = {
      let mut edit_sys = FirstPassEditSystem::new(&self.dictionary, &self.rules, self.rules.next_open_mid());
      edit_sys.allow_anchors = self.allow_anchors;
      edit_sys.allow_context = self.allow_context;
      edit_sys.allow_anti_anchors = self.allow_anti_anchors;
      let frequencies: Vec<f64> = self.dictionary.words.iter().map(|w| w.frequency).collect();
      let r = genastarlike::init_ref_data(est_sys, &edit_sys, &frequencies);
      
      match parallel {
        Some(params) => pargenastarlike::search(est_sys, &edit_sys, &r, params),
        None => {
          let mut w = genastarlike::init_working_data();
          loop {
            if let Some(outcome) = genastarlike::step(est_sys, &edit_sys, &r, &mut w, debug) {
              break outcome;
            }
          }
        }
      }
    };
    
    if let Outcome::FoundImprovement(sub, _) = &outcome {
      self.add_rule(sub);
    }
    
    outcome
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  
  #[test]
  fn apply_anterior_into_matches_apply() {
    let list = HLSubstitutionList::set_1();
    let dictionary = crate::dictionary::load_dictionary().unwrap();
    let mut out = vec![];
    for w in dictionary.words.iter().take(2000) {
      let mut word: Vec<AugGlyph> = w.spelling.iter().map(|g| AugGlyph::Real(*g)).collect();
      for sub in &list.substitutions {
        let into = apply_anterior_into(&sub.anterior, sub.mid, &word, &mut out);
        let in_place = sub.anterior.apply(&mut word, sub.mid);
        assert_eq!(into, in_place);
        assert_eq!(out, word, "{:?}", sub);
      }
    }
  }
  use crate::gaussian_astarlike22::GaussianSystem;

  // The first pass through genastarlike should find exactly the rules that
  // astarlike2::IterativeSystem finds.
  fn check_matches_astarlike2(num_words: usize, num_rules: usize) {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(num_words);

    let system = GaussianSystem { scale: 4.0 };

    let empty = || HLSubstitutionList { substitutions: vec![] };
    let mut old = astarlike2::IterativeSystem::setup(&dictionary, empty());
    let mut new = FirstPass::setup(&dictionary, empty());

    for i in 0 .. num_rules {
      let old_rule = match old.find_next_rule(&system, false) {
        astarlike2::Outcome::FoundImprovement(sub, _) => Some(sub),
        astarlike2::Outcome::FailedToFindImprovement(_, _) => None
      };
      let new_rule = match new.find_next_rule(&system, false) {
        Outcome::FoundImprovement(sub, _) => Some(sub),
        Outcome::FailedToFindImprovement(_, _) | Outcome::NoCandidates => None
      };

      println!("{} {:?} {:?}", i, old_rule, new_rule);
      assert_eq!(old_rule, new_rule, "rule {}", i);
      if old_rule.is_none() {
        break;
      }
    }
  }

  #[test]
  fn resume_continues_identically() {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(40);
    let system = GaussianSystem { scale: 4.0 };
    let empty = || HLSubstitutionList { substitutions: vec![] };
    
    let mut straight = FirstPass::setup(&dictionary, empty());
    for _ in 0 .. 10 {
      straight.find_next_rule(&system, false);
    }
    
    let mut first_half = FirstPass::setup(&dictionary, empty());
    for _ in 0 .. 5 {
      first_half.find_next_rule(&system, false);
    }
    let mut resumed = FirstPass::resume(&dictionary, &first_half.rules);
    for _ in 0 .. 5 {
      resumed.find_next_rule(&system, false);
    }
    
    assert_eq!(straight.rules.substitutions, resumed.rules.substitutions);
  }
  
  // The parallel search should find rules as good as the sequential one's.
  // (Not necessarily the same rules: close races can go either way.)
  #[test]
  fn parallel_first_pass() {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(150);
    let system = GaussianSystem { scale: 4.0 };
    let empty = || HLSubstitutionList { substitutions: vec![] };
    let words: Vec<crate::half_rules::ScoringWord> = dictionary.words.iter().map(|w| crate::half_rules::ScoringWord {
      spelling: w.spelling.iter().map(|g| AugGlyph::Real(*g)).collect(),
      pronunciation: w.pronunciation.iter().map(|g| AugGlyph::Real(*g)).collect(),
      frequency: w.frequency
    }).collect();
    
    let mut sequential = FirstPass::setup(&dictionary, empty());
    for _ in 0 .. 12 {
      sequential.find_next_rule(&system, false);
    }
    let sequential_score = crate::half_rules::global_score(&crate::half_rules::flatten(&sequential.rules), &words);
    
    for threads in [1, 3, 8] {
      let mut parallel = FirstPass::setup(&dictionary, empty());
      for _ in 0 .. 12 {
        parallel.find_next_rule_parallel(&system, ParParams::with_threads(threads));
      }
      let parallel_score = crate::half_rules::global_score(&crate::half_rules::flatten(&parallel.rules), &words);
      println!("{} threads: {:.4} (sequential {:.4})", threads, parallel_score, sequential_score);
      println!("  {:?}", parallel.rules.substitutions);
      assert_eq!(parallel.rules.substitutions.len(), 12);
      assert!(parallel_score <= sequential_score * 1.02);
    }
    println!("  sequential: {:?}", sequential.rules.substitutions);
  }
  
  #[test]
  fn matches_astarlike2_small() {
    check_matches_astarlike2(20, 10);
  }

  // Takes about a minute; run with `cargo test --release -- --ignored`.
  #[test]
  #[ignore]
  fn matches_astarlike2_medium() {
    check_matches_astarlike2(300, 30);
  }
}
