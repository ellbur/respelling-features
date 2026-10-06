// The second pass: re-optimizes the half-rules one at a time.
//
// To re-optimize rule k, it is removed and genastarlike searches for the best
// rule to put in its place. The rule sees its input (the spellings with rules
// 0..k applied) and a target (the pronunciations with the inverses of rules
// k+1.. applied, last rule first). The search's score for a word is the
// distance from the rule's output to the target, which is cheap. The inverses
// aren't exact, so a target can be unreachable: running the later rules
// forward on it doesn't give back the pronunciation. Such a word is scored
// instead by running the later rules forward as far as the first stage whose
// target is reachable, and comparing there.
//
// The search's score is only an estimate, so the rule it finds is kept only if
// it improves the global score (half_rules::global_score) over the current
// rule. A position with no helpful rule can end up with no rule at all.

use float_ord::FloatOrd;
use rayon::prelude::*;
use std::collections::HashSet;
use std::sync::Mutex;

use crate::astarlike2::distance;
use crate::dictionary::Dictionary;
use crate::gaussian_astarlike22::GaussianSystem;
use crate::genastarlike::{self, EditSystem, EstimationSystem, Outcome, SubWithImprovement, Table};
use crate::glyphs::{AugGlyph, aug_encode};
use crate::half_rules::{HalfRule, ScoringWord, apply_all, apply_all_copied, apply_all_into, global_score};
use rustc_hash::FxHashMap;

// Stage s is the input to rule s; stage rules.len() is the final output.
pub struct Targets {
  // targets[s][w]: word w's target at stage s.
  pub targets: Vec<Vec<Vec<AugGlyph>>>,
  // reachable[s][w]: whether running rules s.. forward on targets[s][w] gives
  // back the pronunciation. Checked one rule at a time (rule s must turn
  // targets[s][w] into targets[s+1][w]), which can only flag too many words,
  // never too few.
  pub reachable: Vec<Vec<bool>>
}

// A rule's inverse can have several preimages, and the one the greedy inverse
// picks may not be the path a word actually takes: undoing a later rule might
// turn "tu" into "two" when the word "to" actually reaches "tu" another way.
// Then an already-correct word looks wrong to the search. So wherever the
// word's actual state before rule s leads to its target after rule s, that
// state is used as the target instead. For a correctly pronounced word, every
// target is its actual state.
pub fn compute_targets(rules: &[HalfRule], words: &[ScoringWord]) -> Targets {
  let n = rules.len();
  
  // forward[s][w]: word w's actual state at stage s.
  let mut forward: Vec<Vec<Vec<AugGlyph>>> = Vec::with_capacity(n + 1);
  forward.push(words.iter().map(|w| w.spelling.clone()).collect());
  for s in 0 .. n {
    let next = forward[s].par_iter().map(|state| {
      let mut state = state.clone();
      rules[s].apply(&mut state);
      state
    }).collect();
    forward.push(next);
  }
  
  let mut targets: Vec<Vec<Vec<AugGlyph>>> = vec![vec![]; n + 1];
  let mut reachable: Vec<Vec<bool>> = vec![vec![]; n + 1];
  
  targets[n] = words.iter().map(|w| w.pronunciation.clone()).collect();
  reachable[n] = vec![true; words.len()];
  
  for s in (0 .. n).rev() {
    let inverse = rules[s].inverse();
    let (stage, ok): (Vec<Vec<AugGlyph>>, Vec<bool>) = targets[s + 1].par_iter()
      .zip(reachable[s + 1].par_iter())
      .zip(forward[s].par_iter().zip(forward[s + 1].par_iter()))
      .map(|((next, next_ok), (actual, actual_next))| {
        if actual_next == next {
          return (actual.clone(), *next_ok);
        }
        let mut here = next.clone();
        inverse.apply(&mut here);
        let ok = *next_ok && rules[s].apply_copied(&here).as_ref().unwrap_or(&here) == next;
        (here, ok)
      }).unzip();
    targets[s] = stage;
    reachable[s] = ok;
  }
  
  Targets { targets, reachable }
}

#[derive(Clone, Copy, Debug)]
pub struct SecondPassOptions {
  // Score candidates by running the remaining rules forward to the final
  // output, instead of comparing with the target at the next stage.
  pub exact_scoring: bool,
  // Take candidate outputs from the targets at every later stage, not just
  // the next one. Only useful with exact_scoring, since an output taken from
  // a later stage generally doesn't match the next stage's target.
  pub outputs_from_all_stages: bool,
  // Stop a position's search as soon as the leading rule beats the current
  // rule globally, instead of waiting for the best rule.
  pub early_stop: bool,
  // Take a rule that scores the same as the current one if its output has
  // fewer tokens.
  pub accept_direct_ties: bool,
  // Only take candidate outputs from within this many glyphs of where the
  // candidate's key lines up with the target (None: from anywhere).
  pub output_window: Option<usize>,
  // genastarlike's batch sizes: how many candidates are checked at once (in
  // parallel), and against how many more words each.
  pub edit_chunk_size: usize,
  pub steps_chunk_size: usize,
  // Cache exact scores per word.
  pub memoize: bool,
  // Tuple moves search jointly over rules for all the tuple's slots, keeping
  // all of them (JointEditSystem), instead of emptying and refilling them.
  pub joint_tuples: bool,
  // For joint tuples: how many partial tuples each word's candidate
  // generation keeps per slot.
  pub beam: usize,
  // For joint tuples: judge partial tuples by the best single option at the
  // next slot, instead of by keeping the current rules there.
  pub optimistic_beam: bool,
  // Whether candidates may be anchored to the start (^) or end ($) of a word.
  // Anchored rules need ignore rules in the font, which slow shaping.
  pub allow_anchors: bool,
  // Whether candidates may be anti-anchored (~): kept from the start or end
  // of a word by requiring a word glyph there. These need no ignore rules.
  pub allow_anti_anchors: bool,
  // Whether candidates may have lookbehind or lookahead context.
  pub allow_context: bool
}

thread_local! {
  // Scratch space for scoring: a candidate's output, and two buffers for
  // running the later rules on it.
  static SCRATCH: std::cell::RefCell<(Vec<AugGlyph>, Vec<AugGlyph>, Vec<AugGlyph>)> = std::cell::RefCell::new((vec![], vec![], vec![]));
}

fn distance_slices(a: &[AugGlyph], b: &[AugGlyph]) -> u32 {
  crate::levenshtein::distance(a, b)
}

impl Default for SecondPassOptions {
  fn default() -> SecondPassOptions {
    SecondPassOptions {
      exact_scoring: false,
      outputs_from_all_stages: false,
      early_stop: false,
      accept_direct_ties: false,
      output_window: None,
      edit_chunk_size: 256,
      steps_chunk_size: 256,
      memoize: true,
      joint_tuples: false,
      beam: 32,
      optimistic_beam: false,
      allow_anchors: true,
      allow_anti_anchors: false,
      allow_context: true
    }
  }
}

pub struct SecondPassEditSystem<'a> {
  k: usize,
  rules: &'a [HalfRule],
  inputs: &'a [Vec<AugGlyph>],
  targets: &'a Targets,
  options: SecondPassOptions,
  // The stage each word is compared at: k+1 if its target there is reachable,
  // otherwise the first later stage that is. With exact scoring, always the
  // final stage.
  compare_stage: Vec<usize>,
  // Each word's distance with no rule at position k.
  baseline: Vec<u32>,
  // With exact scoring, each word's cost for outputs already seen. Many
  // candidates (the same change with different contexts) give a word the same
  // output.
  memo: Vec<Mutex<FxHashMap<Vec<AugGlyph>, u32>>>
}

impl<'a> SecondPassEditSystem<'a> {
  pub fn new(k: usize, rules: &'a [HalfRule], inputs: &'a [Vec<AugGlyph>], targets: &'a Targets, options: SecondPassOptions) -> SecondPassEditSystem<'a> {
    let compare_stage: Vec<usize> = (0 .. inputs.len()).map(|w| {
      if options.exact_scoring {
        rules.len()
      }
      else {
        (k + 1 ..= rules.len()).find(|&s| targets.reachable[s][w]).unwrap()
      }
    }).collect();

    let memo = (0 .. inputs.len()).map(|_| Mutex::new(FxHashMap::default())).collect();
    let mut sys = SecondPassEditSystem { k, rules, inputs, targets, options, compare_stage, baseline: vec![], memo };
    sys.baseline = (0 .. inputs.len()).map(|w| sys.cost(w, &inputs[w])).collect();
    sys
  }

  // Distance for word w if rule k's output is `output`.
  fn cost(&self, w: usize, output: &[AugGlyph]) -> u32 {
    let s = self.compare_stage[w];
    let target = &self.targets.targets[s][w];
    if s == self.k + 1 {
      return distance_slices(output, target);
    }
    if self.options.exact_scoring && self.options.memoize {
      if let Some(d) = self.memo[w].lock().unwrap().get(output) {
        return *d;
      }
    }
    let d = SCRATCH.with(|scratch| {
      let (_, a, b) = &mut *scratch.borrow_mut();
      apply_all_into(&self.rules[self.k + 1 .. s], output, a, b);
      distance_slices(a, target)
    });
    if self.options.exact_scoring && self.options.memoize {
      self.memo[w].lock().unwrap().insert(output.to_vec(), d);
    }
    d
  }
  
  pub fn num_unreachable(&self) -> usize {
    self.targets.reachable[self.k + 1].iter().filter(|&&ok| !ok).count()
  }
}

impl<'a> EditSystem<HalfRule> for SecondPassEditSystem<'a> {
  // Every rule that turns a span of the input into a span of the target at
  // stage k+1 (optionally with context and ^/$), where one side is a single
  // glyph, and that improves this word.
  fn find_improving_edits(&self, w: usize) -> Vec<SubWithImprovement<HalfRule>> {
    let last_stage = if self.options.outputs_from_all_stages { self.rules.len() } else { self.k + 1 };
    let targets = stage_targets(self.targets, w, self.k + 1, last_stage);
    let candidates = slot_candidates(&self.inputs[w], &targets, self.options);
    
    let mut res: Vec<SubWithImprovement<HalfRule>> = candidates.into_par_iter().filter_map(|rule| {
      let new_distance = self.new_distance(&rule, w)?;
      if new_distance < self.baseline[w] {
        Some(SubWithImprovement {
          improvement: self.baseline[w] - new_distance,
          size_cost: rule.size_cost(),
          sub: rule
        })
      }
      else {
        None
      }
    }).collect();
    
    // Output spans come from a HashSet; sort so the search is deterministic.
    res.sort_by(|a, b| a.sub.cmp(&b.sub));
    res
  }
  
  fn distance(&self, w: usize) -> u32 {
    self.baseline[w]
  }

  fn new_distance(&self, rule: &HalfRule, w: usize) -> Option<u32> {
    // The rule's output goes in a scratch buffer, which cost() doesn't use.
    let output = SCRATCH.with(|scratch| {
      let (out, _, _) = &mut *scratch.borrow_mut();
      if rule.apply_into(&self.inputs[w], out) { Some(std::mem::take(out)) } else { None }
    })?;
    let d = self.cost(w, &output);
    SCRATCH.with(|scratch| { scratch.borrow_mut().0 = output; });
    Some(d)
  }

  fn describe_word(&self, w: usize) -> String {
    aug_encode(&self.inputs[w])
  }
}

// Broadening: the candidates for rule k (`current`, which has context) are
// copies of it with glyphs added to its context sets. For a word it gets
// wrong, wherever the rule's key and ^/$/~ constraints match its input but
// the context doesn't, adding the glyphs that don't match gives a candidate
// that now matches there; it's kept if it improves the word. Scores are
// exact, and relative to the current rule.
pub struct BroadenEditSystem<'a> {
  k: usize,
  rules: &'a [HalfRule],
  inputs: &'a [Vec<AugGlyph>],
  words: &'a [ScoringWord],
  // Each word's distance with the current rule.
  baseline: Vec<u32>,
  // Each word's distance for outputs of rule k already seen.
  memo: Vec<Mutex<FxHashMap<Vec<AugGlyph>, u32>>>
}

impl<'a> BroadenEditSystem<'a> {
  pub fn new(k: usize, rules: &'a [HalfRule], inputs: &'a [Vec<AugGlyph>], words: &'a [ScoringWord]) -> BroadenEditSystem<'a> {
    let memo = (0 .. inputs.len()).map(|_| Mutex::new(FxHashMap::default())).collect();
    let mut sys = BroadenEditSystem { k, rules, inputs, words, baseline: vec![], memo };
    sys.baseline = (0 .. inputs.len()).into_par_iter().map(|w| {
      let mut output = inputs[w].clone();
      rules[k].apply(&mut output);
      sys.cost(w, &output)
    }).collect();
    sys
  }

  // Distance for word w if rule k's output is `output`.
  fn cost(&self, w: usize, output: &[AugGlyph]) -> u32 {
    if let Some(d) = self.memo[w].lock().unwrap().get(output) {
      return *d;
    }
    let d = SCRATCH.with(|scratch| {
      let (_, a, b) = &mut *scratch.borrow_mut();
      apply_all_into(&self.rules[self.k + 1 ..], output, a, b);
      distance_slices(a, &self.words[w].pronunciation)
    });
    self.memo[w].lock().unwrap().insert(output.to_vec(), d);
    d
  }
}

// The broadenings of `rule` that match `word` somewhere the rule doesn't
// only because of its context (see BroadenEditSystem).
pub fn broadenings(rule: &HalfRule, word: &[AugGlyph]) -> Vec<HalfRule> {
  let (p, a, q) = (rule.pre.len(), rule.at.len(), rule.post.len());
  let mut res = vec![];
  if p + q == 0 || word.len() < p + a + q {
    return res;
  }
  for pos in p ..= word.len() - a - q {
    if word[pos .. pos + a] != rule.at[..] {
      continue;
    }
    let before_is_word = pos > p && word[pos - p - 1].is_word_glyph();
    let after_is_word = pos + a + q < word.len() && word[pos + a + q].is_word_glyph();
    if (rule.at_start && before_is_word) || (rule.not_at_start && !before_is_word)
      || (rule.at_end && after_is_word) || (rule.not_at_end && !after_is_word) {
      continue;
    }
    let mut broader = rule.clone();
    let mut changed = false;
    let mut add = |set: &mut Vec<AugGlyph>, g: AugGlyph| {
      if let Err(i) = set.binary_search(&g) {
        set.insert(i, g);
        changed = true;
      }
    };
    for i in 0 .. p {
      add(&mut broader.pre[i], word[pos - p + i]);
    }
    for i in 0 .. q {
      add(&mut broader.post[i], word[pos + a + i]);
    }
    if changed {
      res.push(broader);
    }
  }
  res.sort();
  res.dedup();
  res
}

impl<'a> EditSystem<HalfRule> for BroadenEditSystem<'a> {
  fn find_improving_edits(&self, w: usize) -> Vec<SubWithImprovement<HalfRule>> {
    if self.baseline[w] == 0 {
      return vec![];
    }
    let current_size = self.rules[self.k].size_cost();
    broadenings(&self.rules[self.k], &self.inputs[w]).into_iter().filter_map(|rule| {
      let new_distance = self.new_distance(&rule, w)?;
      if new_distance < self.baseline[w] {
        Some(SubWithImprovement {
          improvement: self.baseline[w] - new_distance,
          size_cost: rule.size_cost() - current_size,
          sub: rule
        })
      }
      else {
        None
      }
    }).collect()
  }

  fn distance(&self, w: usize) -> u32 {
    self.baseline[w]
  }

  // A broadening matches everywhere the current rule does, so where it
  // doesn't apply, neither does the current rule: None (no change).
  fn new_distance(&self, rule: &HalfRule, w: usize) -> Option<u32> {
    let output = SCRATCH.with(|scratch| {
      let (out, _, _) = &mut *scratch.borrow_mut();
      if rule.apply_into(&self.inputs[w], out) { Some(std::mem::take(out)) } else { None }
    })?;
    let d = self.cost(w, &output);
    SCRATCH.with(|scratch| { scratch.borrow_mut().0 = output; });
    Some(d)
  }

  fn describe_word(&self, w: usize) -> String {
    aug_encode(&self.inputs[w])
  }
}

// The search for a joint tuple move. The tuple's slots (positions in the
// rule list, in increasing order) all keep a rule; a candidate is a list of
// rules for them, and its baseline is the current rules, so the search looks
// only for tuples that beat what's there. Scores are exact.
//
// Candidates for a word come from a beam search over the slots: each
// partial tuple is extended with every single-rule candidate for the word's
// state at the next slot (plus the current rule there), scored by completing
// it with the current rules for the remaining slots, and the best `beam` are
// kept, always including the all-current one. At the last slot the scores
// are exact, and the tuples that improve the word are its candidates.
pub struct JointEditSystem<'a> {
  slots: Vec<usize>,
  rules: &'a [HalfRule],
  // Each word's state at the first slot.
  inputs: Vec<Vec<AugGlyph>>,
  targets: &'a Targets,
  options: SecondPassOptions,
  // Each word's distance with the current rules.
  baseline: Vec<u32>,
  // Per word: (slot index i, state just after slot i's rule) -> distance
  // when the rest of the rules are the current ones.
  memo: Vec<Mutex<FxHashMap<(usize, Vec<AugGlyph>), u32>>>,
  // Per word, for optimistic_beam: (slot index i, state at slot i) -> the
  // best distance any single option at slot i gives, the rest current.
  lookahead_memo: Vec<Mutex<FxHashMap<(usize, Vec<AugGlyph>), u32>>>
}

impl<'a> JointEditSystem<'a> {
  pub fn new(slots: &[usize], rules: &'a [HalfRule], words: &[ScoringWord], targets: &'a Targets, options: SecondPassOptions) -> JointEditSystem<'a> {
    let inputs = inputs_at(rules, words, slots[0]);
    let baseline = words.par_iter().map(|w| distance(&apply_all_copied(rules, &w.spelling), &w.pronunciation)).collect();
    let memo = (0 .. words.len()).map(|_| Mutex::new(FxHashMap::default())).collect();
    let lookahead_memo = (0 .. words.len()).map(|_| Mutex::new(FxHashMap::default())).collect();
    JointEditSystem { slots: slots.to_vec(), rules, inputs, targets, options, baseline, memo, lookahead_memo }
  }
  
  fn pronunciation(&self, w: usize) -> &Vec<AugGlyph> {
    &self.targets.targets[self.rules.len()][w]
  }
  
  // The word's distance, given its state just after slot i's rule, with the
  // current rules everywhere after.
  fn complete(&self, w: usize, i: usize, state: &[AugGlyph]) -> u32 {
    let key = (i, state.to_vec());
    if let Some(d) = self.memo[w].lock().unwrap().get(&key) {
      return *d;
    }
    let d = distance(&apply_all_copied(&self.rules[self.slots[i] + 1 ..], &state.to_vec()), self.pronunciation(w));
    self.memo[w].lock().unwrap().insert(key, d);
    d
  }
  
  // The options for slot i when the word's state there is `state`: its
  // single-rule candidates plus the current rule.
  fn slot_options(&self, w: usize, i: usize, state: &[AugGlyph]) -> Vec<HalfRule> {
    let k = self.slots[i];
    let last_stage = if self.options.outputs_from_all_stages { self.rules.len() } else { k + 1 };
    let targets = stage_targets(self.targets, w, k + 1, last_stage);
    let mut options = slot_candidates(state, &targets, self.options);
    if !options.contains(&self.rules[k]) {
      options.push(self.rules[k].clone());
    }
    options
  }
  
  // For optimistic_beam: the best distance the word can reach from `state`
  // at slot i with any single option there and the current rules after.
  fn lookahead(&self, w: usize, i: usize, state: &[AugGlyph]) -> u32 {
    let key = (i, state.to_vec());
    if let Some(d) = self.lookahead_memo[w].lock().unwrap().get(&key) {
      return *d;
    }
    let d = self.slot_options(w, i, state).into_par_iter().map(|rule| {
      let mut after = state.to_vec();
      rule.apply(&mut after);
      self.complete(w, i, &after)
    }).min().unwrap();
    self.lookahead_memo[w].lock().unwrap().insert(key, d);
    d
  }
  
  // From the state just after slot i's rule to the state at slot i+1.
  fn between(&self, i: usize, state: &mut Vec<AugGlyph>) {
    apply_all(&self.rules[self.slots[i] + 1 .. self.slots[i + 1]], state);
  }
  
  fn size_cost(&self, tuple: &[HalfRule]) -> f64 {
    let current: f64 = self.slots.iter().map(|&s| self.rules[s].size_cost()).sum();
    tuple.iter().map(|r| r.size_cost()).sum::<f64>() - current
  }
}

impl<'a> EditSystem<Vec<HalfRule>> for JointEditSystem<'a> {
  fn find_improving_edits(&self, w: usize) -> Vec<SubWithImprovement<Vec<HalfRule>>> {
    let n = self.slots.len();
    let last_stage = self.rules.len();
    // (rules chosen so far, state at the next slot, score, all current?)
    let mut beam: Vec<(Vec<HalfRule>, Vec<AugGlyph>, u32, bool)> = vec![(vec![], self.inputs[w].clone(), self.baseline[w], true)];
    
    let _ = last_stage;
    for i in 0 .. n {
      let k = self.slots[i];
      
      let mut next: Vec<(Vec<HalfRule>, Vec<AugGlyph>, u32, bool)> = beam.par_iter().flat_map(|(chosen, state, _, all_current)| {
        let options = self.slot_options(w, i, state);
        options.into_par_iter().map(|rule| {
          let mut after = state.clone();
          rule.apply(&mut after);
          // A partial tuple's score: completed with the current rules, or,
          // with optimistic_beam, with the best single option at the next
          // slot (never worse, since the current rule is an option). At the
          // last slot it's exact either way.
          let mut score = self.complete(w, i, &after);
          if i + 1 < n {
            self.between(i, &mut after);
            if self.options.optimistic_beam {
              score = score.min(self.lookahead(w, i + 1, &after));
            }
          }
          let is_current = *all_current && rule == self.rules[k];
          let mut chosen = chosen.clone();
          chosen.push(rule);
          (chosen, after, score, is_current)
        }).collect::<Vec<_>>()
      }).collect();
      
      // Among partial tuples that score the same, prefer the ones that change
      // fewer of the current rules, then smaller ones: the least disruptive
      // ways to fix this word, which are likelier not to break others.
      let changed = |chosen: &Vec<HalfRule>| chosen.iter().zip(&self.slots).filter(|(r, &s)| **r != self.rules[s]).count();
      let size = |chosen: &Vec<HalfRule>| FloatOrd(chosen.iter().map(|r| r.size_cost()).sum::<f64>());
      next.sort_by(|a, b| a.2.cmp(&b.2)
        .then_with(|| changed(&a.0).cmp(&changed(&b.0)))
        .then_with(|| size(&a.0).cmp(&size(&b.0)))
        .then_with(|| a.0.cmp(&b.0)));
      let current = next.iter().position(|p| p.3);
      let mut kept: Vec<_> = vec![];
      let mut current_kept = false;
      for (idx, p) in next.into_iter().enumerate() {
        if kept.len() < self.options.beam || Some(idx) == current && !current_kept {
          current_kept = current_kept || Some(idx) == current;
          kept.push(p);
        }
      }
      beam = kept;
    }
    
    beam.into_iter()
      .filter(|p| p.2 < self.baseline[w])
      .map(|(chosen, _, score, _)| SubWithImprovement {
        improvement: self.baseline[w] - score,
        size_cost: self.size_cost(&chosen),
        sub: chosen
      })
      .collect()
  }
  
  fn distance(&self, w: usize) -> u32 {
    self.baseline[w]
  }
  
  fn new_distance(&self, tuple: &Vec<HalfRule>, w: usize) -> Option<u32> {
    let n = self.slots.len();
    let mut state = self.inputs[w].clone();
    for i in 0 .. n {
      tuple[i].apply(&mut state);
      if i + 1 < n {
        self.between(i, &mut state);
      }
    }
    Some(self.complete(w, n - 1, &state))
  }
  
  fn describe_word(&self, w: usize) -> String {
    aug_encode(&self.inputs[w])
  }
}

#[derive(Debug, PartialEq)]
pub enum PositionResult {
  Unchanged,
  Replaced { old: HalfRule, new: HalfRule, score_change: f64 },
  Removed { old: HalfRule, score_change: f64 }
}

#[derive(Debug, Default)]
pub struct SweepStats {
  pub replaced: usize,
  pub removed: usize,
  pub score_before: f64,
  pub score_after: f64,
  // Positions where some word's target at the next stage was unreachable, and
  // how many such words there were in total.
  pub positions_with_unreachable: usize,
  pub total_unreachable: usize,
  // What the search at each position came back with.
  pub search_found_current: usize,
  pub search_found_other_rejected: usize,
  pub search_found_nothing_kept: usize,
  // Rules that scored the same as the current one, and how many of those
  // were taken as more direct (accept_direct_ties).
  pub search_found_equivalent: usize,
  pub direct_ties_taken: usize,
  // Searches repeated with a wider Gaussian, and the width at the end.
  pub retries: usize,
  pub final_scale: f64,
  // Rejected candidates, with how much they'd have raised the global score.
  pub rejected: Vec<(usize, HalfRule, Option<HalfRule>, f64)>
}

// Searches for the best rule to put at position k, ignoring whatever rule is
// there now. Returns it (None if nothing beats having no rule there) and how
// many words had unreachable targets.
//
// With `early_accept`, whenever a new rule takes the lead with a best
// possible score below `threshold`, it is passed to early_accept, and if that
// returns true the search stops there and returns it.
fn search_at<
    T: Table<Estimate, Estimator, HalfRule> + Send + Sync,
    Estimate: Clone + Send + Sync,
    Estimator: Clone + Send + Sync,
    EstSys: EstimationSystem<T, Estimate, Estimator, HalfRule> + Send + Sync
  >(words: &[ScoringWord], rules: &[HalfRule], k: usize, inputs: &[Vec<AugGlyph>], targets: &Targets, options: SecondPassOptions, est_sys: &EstSys, early_accept: Option<(f64, &dyn Fn(&HalfRule) -> bool)>) -> (Option<HalfRule>, usize)
{
  let edit_sys = SecondPassEditSystem::new(k, rules, inputs, targets, options);
  let unreachable = edit_sys.num_unreachable();
  
  let frequencies: Vec<f64> = words.iter().map(|w| w.frequency).collect();
  let mut r = genastarlike::init_ref_data(est_sys, &edit_sys, &frequencies);
  r.edit_chunk_size = options.edit_chunk_size;
  r.steps_chunk_size = options.steps_chunk_size;
  let mut w = genastarlike::init_working_data();
  let mut last_checked: Option<HalfRule> = None;
  let outcome = loop {
    if let Some(outcome) = genastarlike::step(est_sys, &edit_sys, &r, &mut w, false) {
      break outcome;
    }
    if let Some((threshold, accept)) = early_accept {
      if let Some((edit, best_possible, _, _)) = genastarlike::leader(&w, inputs.len()) {
        if best_possible < threshold && last_checked.as_ref() != Some(edit) {
          last_checked = Some(edit.clone());
          if accept(edit) {
            return (Some(edit.clone()), unreachable);
          }
        }
      }
    }
  };
  
  let candidate = match outcome {
    Outcome::FoundImprovement(rule, _) => Some(rule),
    Outcome::FailedToFindImprovement(_, _) | Outcome::NoCandidates => None
  };
  (candidate, unreachable)
}

// The part of the global score that depends on what's at position k, given
// the inputs to position k.
fn score_at(rules: &[HalfRule], words: &[ScoringWord], k: usize, inputs: &[Vec<AugGlyph>], rule: Option<&HalfRule>) -> f64 {
  let rest = &rules[k + 1 ..];
  let distance_cost: f64 = inputs.par_iter().zip(words.par_iter()).map(|(input, word)| {
    let mut output = input.clone();
    if let Some(rule) = rule {
      rule.apply(&mut output);
    }
    apply_all(rest, &mut output);
    word.frequency * (distance(&output, &word.pronunciation) as f64)
  }).sum();
  distance_cost + rule.map_or(0.0, |r| r.size_cost())
}

// Word w's distinct targets at stages first ..= last.
fn stage_targets<'t>(targets: &'t Targets, w: usize, first: usize, last: usize) -> Vec<&'t Vec<AugGlyph>> {
  let mut seen: HashSet<&'t Vec<AugGlyph>> = HashSet::new();
  let mut res = vec![];
  for s in first ..= last {
    let t = &targets.targets[s][w];
    if seen.insert(t) {
      res.push(t);
    }
  }
  res
}

// The single-rule candidates for a word whose state is `input`: every rule
// that turns a span of the input into a span of one of `targets`
// (optionally with context and ^/$), where one side is a single glyph, and,
// with an output window, where the output lines up with the key.
fn slot_candidates(input: &[AugGlyph], targets: &[&Vec<AugGlyph>], options: SecondPassOptions) -> Vec<HalfRule> {
  let mut outputs: HashSet<Vec<AugGlyph>> = HashSet::new();
  if options.output_window.is_none() {
    for target in targets.iter() {
      for o1 in 0 .. target.len() {
        for o2 in o1 + 1 ..= target.len() {
          outputs.insert(target[o1 .. o2].to_vec());
        }
      }
    }
  }
  let single_outputs: Vec<&[AugGlyph]> = outputs.iter().filter(|o| o.len() == 1).map(|o| o.as_slice()).collect();
  let all_outputs: Vec<&[AugGlyph]> = outputs.iter().map(|o| o.as_slice()).collect();
  
  // With an output window, a candidate's output must come from the part of
  // a target that its key lines up with.
  let alignments: Vec<(&Vec<AugGlyph>, Vec<(usize, usize)>)> = match options.output_window {
    Some(_) => targets.iter().map(|t| (*t, align_boundaries(input, t))).collect(),
    None => vec![]
  };
  
  (0 .. input.len()).into_par_iter().map(|k1| {
    let mut res = vec![];
    for k2 in k1 + 1 ..= input.len() {
      // A key at a word boundary can be anchored there (^/$); one inside a
      // word can be anti-anchored (~). "at_start" below means whichever of
      // the two applies.
      let start_is_boundary = k1 == 0 || !input[k1 - 1].is_word_glyph();
      let end_is_boundary = k2 == input.len() || !input[k2].is_word_glyph();
      let can_be_at_start = if start_is_boundary { options.allow_anchors } else { options.allow_anti_anchors };
      let can_be_at_end = if end_is_boundary { options.allow_anchors } else { options.allow_anti_anchors };
      let whole = &input[k1 .. k2];
      
      for s1 in 0 .. whole.len() {
        for s2 in s1 + 1 ..= whole.len() {
          if !options.allow_context && (s1 > 0 || s2 < whole.len()) {
            continue;
          }
          let at = &whole[s1 .. s2];
          let windowed: Vec<&[AugGlyph]>;
          let outputs: &Vec<&[AugGlyph]> = match options.output_window {
            None => if at.len() == 1 { &all_outputs } else { &single_outputs },
            Some(window) => {
              windowed = windowed_outputs(&alignments, k1 + s1, k1 + s2, window, at.len() == 1);
              &windowed
            }
          };
          
          for &at_start in if can_be_at_start { &[true, false][..] } else { &[false][..] } {
            for &at_end in if can_be_at_end { &[true, false][..] } else { &[false][..] } {
              for out in outputs.iter() {
                // A rule that writes back what it matched does nothing.
                if *out == at {
                  continue;
                }
                res.push(HalfRule {
                  pre: crate::half_rules::singletons(&whole[.. s1]),
                  at: at.to_vec(),
                  post: crate::half_rules::singletons(&whole[s2 ..]),
                  at_start: at_start && start_is_boundary,
                  at_end: at_end && end_is_boundary,
                  not_at_start: at_start && !start_is_boundary,
                  not_at_end: at_end && !end_is_boundary,
                  out: out.to_vec()
                });
              }
            }
          }
        }
      }
    }
    res
  }).flatten().collect()
}

// For each boundary between glyphs of `a` (0 ..= a.len()), the range of
// boundaries of `b` it lines up with in a minimal Levenshtein alignment (a
// range when glyphs of `b` are inserted there).
fn align_boundaries(a: &[AugGlyph], b: &[AugGlyph]) -> Vec<(usize, usize)> {
  let (n, m) = (a.len(), b.len());
  let mut d = vec![vec![0u32; m + 1]; n + 1];
  for i in 0 ..= n { d[i][0] = i as u32; }
  for j in 0 ..= m { d[0][j] = j as u32; }
  for i in 1 ..= n {
    for j in 1 ..= m {
      let sub = d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 };
      d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
    }
  }
  let mut map = vec![(usize::MAX, 0); n + 1];
  let (mut i, mut j) = (n, m);
  loop {
    map[i] = (map[i].0.min(j), map[i].1.max(j));
    if i == 0 && j == 0 {
      break;
    }
    if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 } {
      i -= 1;
      j -= 1;
    }
    else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
      i -= 1;
    }
    else {
      j -= 1;
    }
  }
  map
}

// Outputs for a key covering input glyphs a1 .. a2: spans of each target
// that start within `window` of where a1 lines up and end within `window` of
// where a2 lines up. A key of more than one glyph can only have a
// single-glyph output.
fn windowed_outputs<'t>(alignments: &[(&'t Vec<AugGlyph>, Vec<(usize, usize)>)], a1: usize, a2: usize, window: usize, any_length: bool) -> Vec<&'t [AugGlyph]> {
  let mut res: HashSet<&'t [AugGlyph]> = HashSet::new();
  for (target, map) in alignments {
    let (c1, c2) = (map[a1].0, map[a2].1);
    for o1 in c1.saturating_sub(window) ..= (c1 + window).min(target.len()) {
      for o2 in c2.saturating_sub(window) ..= (c2 + window).min(target.len()) {
        if o1 < o2 && (any_length || o2 == o1 + 1) {
          res.insert(&target[o1 .. o2]);
        }
      }
    }
  }
  let mut res: Vec<&'t [AugGlyph]> = res.into_iter().collect();
  res.sort();
  res
}

fn num_tokens(rule: &HalfRule) -> usize {
  rule.out.iter().filter(|g| matches!(g, AugGlyph::Synthetic(_))).count()
}

fn inputs_at(rules: &[HalfRule], words: &[ScoringWord], k: usize) -> Vec<Vec<AugGlyph>> {
  words.par_iter().map(|w| apply_all_copied(&rules[.. k], &w.spelling)).collect()
}

// The later rules that read a token rule k outputs, e.g. an anterior's
// posterior.
pub fn partners(rules: &[HalfRule], k: usize) -> Vec<usize> {
  let tokens: Vec<AugGlyph> = rules[k].out.iter().filter(|g| matches!(g, AugGlyph::Synthetic(_))).cloned().collect();
  (k + 1 .. rules.len()).filter(|&j| {
    let r = &rules[j];
    r.pre.iter().flatten().chain(r.at.iter()).chain(r.post.iter().flatten()).any(|g| tokens.contains(g))
  }).collect()
}

struct TupleEval {
  improved: Option<(Vec<HalfRule>, TupleChange)>,
  // The search's result was worse than the current tuple (joint tuples).
  failed: bool
}

#[derive(Debug)]
pub struct TupleChange {
  pub slots: Vec<usize>,
  pub old: Vec<HalfRule>,
  pub new: Vec<Option<HalfRule>>,
  pub score_change: f64
}

// Sets of n rules starting at rule k that are connected through tokens: each
// rule after k is a partner (see partners) of an earlier one in the set.
pub fn tuples_from(rules: &[HalfRule], k: usize, n: usize) -> Vec<Vec<usize>> {
  let mut level: Vec<Vec<usize>> = vec![vec![k]];
  for _ in 1 .. n {
    let mut next: std::collections::BTreeSet<Vec<usize>> = std::collections::BTreeSet::new();
    for set in &level {
      for &m in set {
        for j in partners(rules, m) {
          if !set.contains(&j) {
            let mut bigger = set.clone();
            bigger.push(j);
            bigger.sort();
            next.insert(bigger);
          }
        }
      }
    }
    level = next.into_iter().collect();
  }
  level
}

// Adapting the Gaussian estimator's width as sweeps go around the rule set.
// A search that returns a rule worse than the current one stopped exploring
// too early: the width widens. A search whose rule is accepted narrows it.
// Returning the current rule or an equivalent one leaves it alone.
//
// By default the width is a running average for the whole rule set: failed
// positions are not searched again, and the factors are small, so it hovers
// where failures are rare compared to accepted changes (for 1.2 and 0.9,
// about 0.58 failures per accepted change), and creeps up as improvements get
// harder to find. With `retry`, a failed position is instead searched again,
// wider, until it succeeds or the width reaches `max`.
#[derive(Clone, Copy, Debug)]
pub struct AdaptiveScale {
  pub widen: f64,
  pub narrow: f64,
  pub min: f64,
  pub max: f64,
  pub retry: bool
}

impl Default for AdaptiveScale {
  fn default() -> AdaptiveScale {
    AdaptiveScale { widen: 1.2, narrow: 0.9, min: 0.125, max: 16.0, retry: false }
  }
}

impl AdaptiveScale {
  // The earlier per-position scheme: double and retry on failure.
  pub fn per_position() -> AdaptiveScale {
    AdaptiveScale { widen: 2.0, narrow: 0.8, min: 0.125, max: 16.0, retry: true }
  }
}

pub struct Checkpoint {
  pub rules: Vec<HalfRule>,
  pub scale: Option<f64>,
  pub position: usize,
  // Set if a tuple sweep was under way: its tuple size and position.
  pub tuple_position: Option<(usize, usize)>,
  // How many of the tuples from that position's rule were already
  // evaluated without improving.
  pub tuple_skip: usize,
  // Set once broadening has started (position is then a broadening sweep's).
  pub broadening: bool
}

pub struct SecondPass {
  pub words: Vec<ScoringWord>,
  pub rules: Vec<HalfRule>,
  pub options: SecondPassOptions,
  // The Gaussian estimator's width (GaussianSystem::scale).
  pub scale: f64,
  pub adaptive: Option<AdaptiveScale>,
  // Print a progress line every this many positions of a sweep (0: never).
  pub progress_every: usize,
  // Where save_checkpoint writes.
  pub checkpoint_path: Option<String>,
  // The position the current sweep has reached; the next sweep starts here
  // (then goes back to 0), so a resumed run carries on mid-sweep.
  pub sweep_position: usize,
  // Likewise for a tuple sweep: its tuple size and position, and how many
  // of the tuples from the rule at that position were already evaluated
  // without improving (one rule can have hours' worth).
  pub tuple_position: Option<(usize, usize)>,
  pub tuple_skip: usize,
  // Whether the sweeps under way are broadening sweeps (broaden_sweep),
  // which come after the others have converged.
  pub broadening: bool
}

impl SecondPass {
  pub fn new(dictionary: &Dictionary, rules: Vec<HalfRule>) -> SecondPass {
    let aug = |gs: &Vec<crate::glyphs::Glyph>| gs.iter().map(|g| AugGlyph::Real(*g)).collect();
    SecondPass {
      words: dictionary.words.iter().map(|w| ScoringWord {
        spelling: aug(&w.spelling),
        pronunciation: aug(&w.pronunciation),
        frequency: w.frequency
      }).collect(),
      rules,
      options: SecondPassOptions::default(),
      scale: 4.0,
      adaptive: None,
      progress_every: 0,
      checkpoint_path: None,
      sweep_position: 0,
      tuple_position: None,
      tuple_skip: 0,
      broadening: false
    }
  }

  // A checkpoint file: "# scale <width>" and "# position <k>" (where the
  // current sweep had got to), "# tuple <n> <k> [<skip>]" during a tuple
  // sweep, and
  // "# broaden" once broadening has started, then one half-rule per line.
  pub fn load_checkpoint(path: &str) -> Option<Checkpoint> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut checkpoint = Checkpoint { rules: vec![], scale: None, position: 0, tuple_position: None, tuple_skip: 0, broadening: false };
    for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
      if let Some(v) = line.strip_prefix("# scale ") {
        checkpoint.scale = Some(v.parse::<f64>().unwrap());
      }
      else if let Some(v) = line.strip_prefix("# position ") {
        checkpoint.position = v.parse::<usize>().unwrap();
      }
      else if let Some(v) = line.strip_prefix("# tuple ") {
        let fields: Vec<usize> = v.split_whitespace().map(|f| f.parse().unwrap()).collect();
        checkpoint.tuple_position = Some((fields[0], fields[1]));
        checkpoint.tuple_skip = fields.get(2).cloned().unwrap_or(0);
      }
      else if line == "# broaden" {
        checkpoint.broadening = true;
      }
      else {
        checkpoint.rules.push(HalfRule::decode(line).unwrap());
      }
    }
    Some(checkpoint)
  }
  
  // Writes the current rules to checkpoint_path, if set. Called whenever the
  // rules change, and after every position of a sweep, so the file always
  // has the best rules found so far and where the sweep is. Goes through a
  // temporary file so an interruption can't leave it half written.
  pub fn save_checkpoint(&self) {
    self.write_checkpoint(self.tuple_position, self.tuple_skip);
  }
  
  // save_checkpoint, with the given tuple sweep position.
  fn write_checkpoint(&self, tuple_position: Option<(usize, usize)>, tuple_skip: usize) {
    let Some(path) = &self.checkpoint_path else { return };
    let mut text = format!("# scale {}\n# position {}\n", self.scale, self.sweep_position);
    if let Some((n, k)) = tuple_position {
      if tuple_skip > 0 {
        text += &format!("# tuple {} {} {}\n", n, k, tuple_skip);
      }
      else {
        text += &format!("# tuple {} {}\n", n, k);
      }
    }
    if self.broadening {
      text += "# broaden\n";
    }
    for r in &self.rules {
      text += &r.encode();
      text += "\n";
    }
    let tmp = format!("{}.tmp", path);
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(&tmp, path).unwrap();
  }
  
  pub fn score(&self) -> f64 {
    global_score(&self.rules, &self.words)
  }

  fn score_at(&self, k: usize, inputs: &[Vec<AugGlyph>], rule: Option<&HalfRule>) -> f64 {
    score_at(&self.rules, &self.words, k, inputs, rule)
  }
  
  // Re-optimizes rule k and updates self.rules. `inputs` are the words with
  // rules 0..k applied; `targets` must be up to date for stages after k.
  pub fn optimize_position(&mut self, k: usize, inputs: &[Vec<AugGlyph>], targets: &Targets, stats: &mut SweepStats) -> PositionResult {
    let current = self.rules[k].clone();
    let mut first_try = true;
    
    // For early stopping: any rule that beats the current one globally is
    // taken. The search's scores are relative to having no rule at k, so a
    // rule can only beat the current one if its best possible score is below
    // the current rule's.
    let current_score = self.score_at(k, inputs, Some(&current));
    let current_relative = current_score - self.score_at(k, inputs, None);
    let (rules, words) = (&self.rules, &self.words);
    let beats_current = |rule: &HalfRule| rule != &current && score_at(rules, words, k, inputs, Some(rule)) < current_score - 1e-9;
    let early_accept: Option<(f64, &dyn Fn(&HalfRule) -> bool)> =
      if self.options.early_stop { Some((current_relative, &beats_current)) } else { None };
    
    loop {
      let system = GaussianSystem { scale: self.scale };
      let (candidate, unreachable) = search_at(&self.words, &self.rules, k, inputs, targets, self.options, &system, early_accept);
      if first_try && unreachable > 0 {
        stats.positions_with_unreachable += 1;
        stats.total_unreachable += unreachable;
      }
      first_try = false;
      
      if candidate.as_ref() == Some(&current) {
        stats.search_found_current += 1;
        return PositionResult::Unchanged;
      }
      
      let candidate_score = self.score_at(k, inputs, candidate.as_ref());
      let score_change = candidate_score - current_score;
      
      // Require a real improvement, not float noise. A rule that scores the
      // same as the current one usually writes directly what the current
      // rule writes through a token ([th]→ϑ for [th]→{0}); with
      // accept_direct_ties, it's taken if its output has fewer tokens, which
      // frees the token's other rules for other uses without cycling.
      let improves = score_change < -1e-9;
      let tie = score_change.abs() <= 1e-9;
      let fewer_tokens = candidate.as_ref().map_or(0, num_tokens) < num_tokens(&current);
      if improves || (tie && self.options.accept_direct_ties && fewer_tokens) {
        if improves {
          if let Some(a) = self.adaptive {
            self.scale = (self.scale * a.narrow).max(a.min);
          }
        }
        else {
          stats.direct_ties_taken += 1;
        }
        let result = match candidate {
          Some(new) => {
            self.rules[k] = new.clone();
            PositionResult::Replaced { old: current, new, score_change }
          },
          None => {
            self.rules.remove(k);
            PositionResult::Removed { old: current, score_change }
          }
        };
        self.save_checkpoint();
        return result;
      }
      
      // A tie means the search found an equally good rule, not that it
      // stopped too early, so it's no reason to widen.
      if tie {
        stats.search_found_equivalent += 1;
        return PositionResult::Unchanged;
      }
      
      if let Some(a) = self.adaptive {
        if a.retry {
          if self.scale * a.widen <= a.max {
            self.scale *= a.widen;
            stats.retries += 1;
            continue;
          }
        }
        else {
          self.scale = (self.scale * a.widen).min(a.max);
        }
      }
      
      if candidate.is_some() {
        stats.search_found_other_rejected += 1;
      }
      else {
        stats.search_found_nothing_kept += 1;
      }
      stats.rejected.push((k, current, candidate, score_change));
      return PositionResult::Unchanged;
    }
  }
  
  // Re-optimizes the rules at `slots` (in increasing order) together: all
  // of them are emptied, then filled one at a time, first to last, each by
  // searching for the best rule with the later slots still empty. Keeps the
  // result if the global score improves.
  pub fn try_tuple(&mut self, slots: &[usize]) -> Option<TupleChange> {
    let targets = if self.options.joint_tuples { Some(compute_targets(&self.rules, &self.words)) } else { None };
    let eval = self.evaluate_tuple(slots, targets.as_ref());
    self.note_tuple_outcome(&eval);
    let (rules, change) = eval.improved?;
    self.rules = rules;
    self.save_checkpoint();
    Some(change)
  }
  
  // try_tuple without changing anything. Joint tuples need the current
  // rules' targets.
  fn evaluate_tuple(&self, slots: &[usize], targets: Option<&Targets>) -> TupleEval {
    if self.options.joint_tuples {
      self.evaluate_joint(slots, targets.unwrap())
    }
    else {
      TupleEval { improved: self.evaluate_refill(slots), failed: false }
    }
  }
  
  // Joint tuple searches adapt the running-average width like single-rule
  // searches: the current tuple is the search's baseline, so a tuple that
  // fails the global check means the search stopped too early.
  fn note_tuple_outcome(&mut self, eval: &TupleEval) {
    if !self.options.joint_tuples {
      return;
    }
    if let Some(a) = self.adaptive {
      if a.retry {
        return;
      }
      if eval.improved.is_some() {
        self.scale = (self.scale * a.narrow).max(a.min);
      }
      else if eval.failed {
        self.scale = (self.scale * a.widen).min(a.max);
      }
    }
  }
  
  // A joint tuple search: all of the tuple's rules stay, and the search
  // looks for a set of rules for its slots that beats the current ones
  // (see JointEditSystem).
  fn evaluate_joint(&self, slots: &[usize], targets: &Targets) -> TupleEval {
    let edit_sys = JointEditSystem::new(slots, &self.rules, &self.words, targets, self.options);
    let est_sys = GaussianSystem { scale: self.scale };
    let frequencies: Vec<f64> = self.words.iter().map(|w| w.frequency).collect();
    let mut r = genastarlike::init_ref_data(&est_sys, &edit_sys, &frequencies);
    r.edit_chunk_size = self.options.edit_chunk_size;
    r.steps_chunk_size = self.options.steps_chunk_size;
    let mut w = genastarlike::init_working_data();
    let outcome = loop {
      if let Some(outcome) = genastarlike::step(&est_sys, &edit_sys, &r, &mut w, false) {
        break outcome;
      }
    };
    let Outcome::FoundImprovement(new, _) = outcome else {
      return TupleEval { improved: None, failed: false };
    };
    
    let before = self.score();
    let mut rules = self.rules.clone();
    for (i, &s) in slots.iter().enumerate() {
      rules[s] = new[i].clone();
    }
    let after = global_score(&rules, &self.words);
    if after < before - 1e-9 {
      let old = slots.iter().map(|&s| self.rules[s].clone()).collect();
      let change = TupleChange { slots: slots.to_vec(), old, new: new.into_iter().map(Some).collect(), score_change: after - before };
      TupleEval { improved: Some((rules, change)), failed: false }
    }
    else {
      TupleEval { improved: None, failed: after > before + 1e-9 }
    }
  }
  
  // The empty-and-refill tuple move: the new rule list and the change, if
  // the global score improves.
  fn evaluate_refill(&self, slots: &[usize]) -> Option<(Vec<HalfRule>, TupleChange)> {
    let before = self.score();
    let est_sys = &GaussianSystem { scale: self.scale };
    
    let mut filled: Vec<Option<HalfRule>> = self.rules.iter().cloned().map(Some).collect();
    for &s in slots {
      filled[s] = None;
    }
    
    for &s in slots {
      // The rule list with the other empty slots left out, and the old rule
      // at slot s as a placeholder (search_at ignores the rule at the
      // position it searches).
      let mut list = vec![];
      let mut pos = 0;
      for (i, r) in filled.iter().enumerate() {
        if i == s {
          pos = list.len();
          list.push(self.rules[s].clone());
        }
        else if let Some(r) = r {
          list.push(r.clone());
        }
      }
      let inputs = inputs_at(&list, &self.words, pos);
      let targets = compute_targets(&list, &self.words);
      let (candidate, _) = search_at(&self.words, &list, pos, &inputs, &targets, self.options, est_sys, None);
      filled[s] = candidate;
    }
    
    let new: Vec<Option<HalfRule>> = slots.iter().map(|&s| filled[s].clone()).collect();
    let rules: Vec<HalfRule> = filled.into_iter().flatten().collect();
    let after = global_score(&rules, &self.words);
    if after < before - 1e-9 {
      let old = slots.iter().map(|&s| self.rules[s].clone()).collect();
      Some((rules, TupleChange { slots: slots.to_vec(), old, new, score_change: after - before }))
    }
    else {
      None
    }
  }
  
  // Tries try_tuple on every connected tuple of n rules (see tuples_from).
  // Returns the changes made.
  pub fn tuple_sweep(&mut self, n: usize, verbose: bool) -> Vec<TupleChange> {
    let mut changes = vec![];
    let mut k = self.resume_tuple_position(n);
    while k < self.rules.len() {
      self.note_tuple_position(n, k, 0);
      let mut changed = false;
      for slots in tuples_from(&self.rules, k, n) {
        if let Some(change) = self.try_tuple(&slots) {
          if verbose {
            println!("{:?}: {:?} -> {:?} ({:+.4})", change.slots, change.old, change.new, change.score_change);
          }
          changes.push(change);
          changed = true;
          break;
        }
      }
      // After a change, look at position k again, since its rule and
      // partners have changed.
      if !changed {
        k += 1;
      }
    }
    self.finish_tuple_sweep();
    changes
  }
  
  // Where a tuple sweep of size n starts: where a resumed one had got to, or
  // at the beginning.
  fn resume_tuple_position(&self, n: usize) -> usize {
    match self.tuple_position {
      Some((m, k)) if m == n => k,
      _ => 0
    }
  }
  
  fn note_tuple_position(&mut self, n: usize, k: usize, skip: usize) {
    if self.tuple_position != Some((n, k)) || self.tuple_skip != skip {
      self.tuple_position = Some((n, k));
      self.tuple_skip = skip;
      self.save_checkpoint();
    }
  }
  
  fn finish_tuple_sweep(&mut self) {
    self.tuple_position = None;
    self.tuple_skip = 0;
    self.save_checkpoint();
  }
  
  pub fn pair_sweep(&mut self, verbose: bool) -> Vec<TupleChange> {
    self.tuple_sweep(2, verbose)
  }
  
  // tuple_sweep, trying tuples on `threads` threads at once. Each thread
  // takes the next tuple in tuple_sweep's order and runs its searches alone
  // (in its own one-thread rayon pool). Once some tuple improves the score,
  // no tuples after it are started; when the ones before it have finished
  // without improving, it is applied, and the sweep carries on from its
  // first rule as tuple_sweep would. Gives the same result as tuple_sweep.
  pub fn tuple_sweep_parallel(&mut self, n: usize, threads: usize, verbose: bool) -> Vec<TupleChange> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    // While a batch runs, how often the checkpoint records how far it has
    // got, so a long stretch without improvements isn't lost to a restart.
    const PROGRESS_INTERVAL: Duration = Duration::from_secs(60);
    
    let pools: Vec<rayon::ThreadPool> = (0 .. threads).map(|_| {
      rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap()
    }).collect();
    
    let mut changes = vec![];
    let mut k = self.resume_tuple_position(n);
    // Tuples from rule k evaluated before a restart.
    let mut skip = if self.tuple_position.map(|(m, _)| m) == Some(n) { self.tuple_skip } else { 0 };
    while k < self.rules.len() {
      self.note_tuple_position(n, k, skip);
      // Every tuple from rule k on, in tuple_sweep's order, and the rule
      // each was generated from.
      let (mut queue, mut origins): (Vec<Vec<usize>>, Vec<usize>) = (k .. self.rules.len())
        .flat_map(|m| tuples_from(&self.rules, m, n).into_iter().map(move |t| (t, m)))
        .unzip();
      let skipped = skip.min(origins.iter().take_while(|&&m| m == k).count());
      queue.drain(.. skipped);
      origins.drain(.. skipped);
      skip = 0;
      if queue.is_empty() {
        break;
      }
      
      let next = AtomicUsize::new(0);
      let done: Vec<AtomicBool> = (0 .. queue.len()).map(|_| AtomicBool::new(false)).collect();
      let (finished_tx, finished_rx) = mpsc::channel::<()>();
      // The earliest tuple known to improve (queue.len() if none).
      let first_improving = AtomicUsize::new(queue.len());
      let results: Mutex<Vec<(usize, TupleEval)>> = Mutex::new(vec![]);
      let targets = if self.options.joint_tuples { Some(compute_targets(&self.rules, &self.words)) } else { None };
      let targets = targets.as_ref();
      let this = &*self;
      
      std::thread::scope(|scope| {
        for pool in &pools {
          let (queue, next, first_improving, results, done) = (&queue, &next, &first_improving, &results, &done);
          let finished_tx = finished_tx.clone();
          scope.spawn(move || {
            pool.install(|| {
              loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= queue.len() || i > first_improving.load(Ordering::SeqCst) {
                  break;
                }
                let eval = this.evaluate_tuple(&queue[i], targets);
                if eval.improved.is_some() {
                  first_improving.fetch_min(i, Ordering::SeqCst);
                }
                results.lock().unwrap().push((i, eval));
                done[i].store(true, Ordering::SeqCst);
              }
            });
            let _ = finished_tx.send(());
          });
        }
        
        // So the channel disconnects if every worker is gone, even by panic.
        drop(finished_tx);
        
        // Every PROGRESS_INTERVAL, checkpoint the first tuple not yet
        // evaluated (none before it having improved), as its rule and how
        // many of that rule's tuples come before it.
        let mut finished = 0;
        let mut prefix = 0;
        let mut saved = (k, skipped);
        let mut next_save = Instant::now() + PROGRESS_INTERVAL;
        while finished < pools.len() {
          match finished_rx.recv_timeout(next_save.saturating_duration_since(Instant::now())) {
            Ok(()) => finished += 1,
            Err(mpsc::RecvTimeoutError::Timeout) => {
              let limit = first_improving.load(Ordering::SeqCst).min(queue.len());
              while prefix < limit && done[prefix].load(Ordering::SeqCst) {
                prefix += 1;
              }
              if prefix < queue.len() {
                let origin = origins[prefix];
                let first_of_origin = origins.partition_point(|&m| m < origin);
                let within = prefix - first_of_origin + if origin == k { skipped } else { 0 };
                if (origin, within) > saved {
                  saved = (origin, within);
                  this.write_checkpoint(Some((n, origin)), within);
                }
              }
              next_save = Instant::now() + PROGRESS_INTERVAL;
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => break
          }
        }
      });
      
      // Width updates as tuple_sweep would make them, from the tuples up to
      // the first improving one (all of which were evaluated, whatever the
      // timing). They all started from the same width, so with adaptive
      // widths the result can differ from tuple_sweep's.
      let first = first_improving.load(Ordering::SeqCst);
      let mut results: Vec<(usize, TupleEval)> = results.into_inner().unwrap().into_iter().filter(|r| r.0 <= first).collect();
      results.sort_by_key(|r| r.0);
      let mut committed = None;
      for (_, eval) in results {
        self.note_tuple_outcome(&eval);
        if let Some(improved) = eval.improved {
          committed = Some(improved);
          break;
        }
      }
      match committed {
        Some((rules, change)) => {
          if verbose {
            println!("{:?}: {:?} -> {:?} ({:+.4})", change.slots, change.old, change.new, change.score_change);
          }
          k = change.slots[0];
          self.rules = rules;
          self.save_checkpoint();
          changes.push(change);
        },
        None => break
      }
    }
    self.finish_tuple_sweep();
    changes
  }
  
  // Re-optimizes every position once, first to last.
  pub fn sweep(&mut self, verbose: bool) -> SweepStats
  {
    let mut stats = SweepStats { score_before: self.score(), ..Default::default() };

    // Changing rule k doesn't change the targets after stage k, so the targets
    // computed here stay valid for every position still to come.
    let mut targets = compute_targets(&self.rules, &self.words);
    // Start where a resumed sweep left off.
    let mut k = self.sweep_position.min(self.rules.len());
    let mut inputs: Vec<Vec<AugGlyph>> = inputs_at(&self.rules, &self.words, k);
    
    let start = std::time::Instant::now();
    while k < self.rules.len() {
      if k != self.sweep_position {
        self.sweep_position = k;
        self.save_checkpoint();
      }
      if self.progress_every > 0 && k > 0 && k % self.progress_every == 0 {
        println!("  position {}/{}: {} replaced, {} removed so far, {:.0}s", k, self.rules.len(), stats.replaced, stats.removed, start.elapsed().as_secs_f64());
      }
      let result = self.optimize_position(k, &inputs, &targets, &mut stats);
      if verbose && result != PositionResult::Unchanged {
        println!("{:>4}: {:?}", k, result);
      }

      match result {
        PositionResult::Removed { .. } => {
          // Old stage k was the target for the removed rule's input; old stage
          // k+1 becomes stage k. The inputs to position k are unchanged.
          targets.targets.remove(k);
          targets.reachable.remove(k);
          stats.removed += 1;
        },
        other => {
          if let PositionResult::Replaced { .. } = other {
            stats.replaced += 1;
          }
          let rule = &self.rules[k];
          inputs.par_iter_mut().for_each(|input| { rule.apply(input); });
          k += 1;
        }
      }
    }

    self.sweep_position = 0;
    self.save_checkpoint();
    stats.score_after = self.score();
    stats.final_scale = self.scale;
    stats
  }
  
  // Tries broadening each rule with context in turn (see
  // BroadenEditSystem): the search finds the best broadening, which replaces
  // the rule if the global score improves. The width adapts as in sweep.
  pub fn broaden_sweep(&mut self, verbose: bool) -> SweepStats {
    let mut stats = SweepStats { score_before: self.score(), ..Default::default() };
    if !self.broadening {
      self.broadening = true;
      self.sweep_position = 0;
      self.save_checkpoint();
    }
    let mut k = self.sweep_position.min(self.rules.len());
    let mut inputs: Vec<Vec<AugGlyph>> = inputs_at(&self.rules, &self.words, k);
    let frequencies: Vec<f64> = self.words.iter().map(|w| w.frequency).collect();
    let start = std::time::Instant::now();
    while k < self.rules.len() {
      if k != self.sweep_position {
        self.sweep_position = k;
        self.save_checkpoint();
      }
      if self.progress_every > 0 && k > 0 && k % self.progress_every == 0 {
        println!("  position {}/{}: {} broadened so far, {:.0}s", k, self.rules.len(), stats.replaced, start.elapsed().as_secs_f64());
      }
      let current = self.rules[k].clone();
      if !current.pre.is_empty() || !current.post.is_empty() {
        let candidate = {
          let edit_sys = BroadenEditSystem::new(k, &self.rules, &inputs, &self.words);
          let system = GaussianSystem { scale: self.scale };
          let mut r = genastarlike::init_ref_data(&system, &edit_sys, &frequencies);
          r.edit_chunk_size = self.options.edit_chunk_size;
          r.steps_chunk_size = self.options.steps_chunk_size;
          let mut working = genastarlike::init_working_data();
          let outcome = loop {
            if let Some(outcome) = genastarlike::step(&system, &edit_sys, &r, &mut working, false) {
              break outcome;
            }
          };
          match outcome {
            Outcome::FoundImprovement(rule, _) => Some(rule),
            _ => None
          }
        };
        if let Some(candidate) = candidate {
          let current_score = self.score_at(k, &inputs, Some(&current));
          let score_change = self.score_at(k, &inputs, Some(&candidate)) - current_score;
          if score_change < -1e-9 {
            if verbose {
              println!("{:>4}: {:?} -> {:?} ({:+.4})", k, current, candidate, score_change);
            }
            if let Some(a) = self.adaptive {
              self.scale = (self.scale * a.narrow).max(a.min);
            }
            self.rules[k] = candidate;
            stats.replaced += 1;
            self.save_checkpoint();
          }
          else if score_change > 1e-9 {
            if let Some(a) = self.adaptive {
              self.scale = (self.scale * a.widen).min(a.max);
            }
            stats.search_found_other_rejected += 1;
          }
        }
      }
      let rule = &self.rules[k];
      inputs.par_iter_mut().for_each(|input| { rule.apply(input); });
      k += 1;
    }
    self.sweep_position = 0;
    self.save_checkpoint();
    stats.score_after = self.score();
    stats.final_scale = self.scale;
    stats
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::dictionary::DictionaryWord;
  use crate::glyphs::decode;

  fn dictionary(words: &[(&str, &str)]) -> Dictionary {
    Dictionary {
      words: words.iter().map(|(s, p)| DictionaryWord {
        spelling: decode(s),
        pronunciation: decode(p),
        frequency: 1.0
      }).collect()
    }
  }

  fn rules(text: &str) -> Vec<HalfRule> {
    text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).map(|l| HalfRule::decode(l).unwrap()).collect()
  }

  #[test]
  fn fixes_a_wrong_vowel() {
    // The posterior gives the wrong vowel. The target for position 0 is "cæt"
    // (undoing [c]→k), so the anterior is replaced by [a]→æ directly, and the
    // posterior, which no longer does anything, is removed.
    let d = dictionary(&[("cat", "kæt"), ("hat", "hæt"), ("sat", "sæt")]);
    let mut pass = SecondPass::new(&d, rules("[a]→{0}\n[c]→k\n[{0}]→ɑ"));
    let before = pass.score();
    let stats = pass.sweep(true);
    assert_eq!(pass.rules, rules("[a]→æ\n[c]→k"));
    assert!(stats.score_after < before);
    assert_eq!(stats.score_after, pass.score());
  }

  #[test]
  fn broadenings_add_the_mismatched_glyphs() {
    use crate::glyphs::aug_decode;
    let r = HalfRule::decode("a[t]e→d").unwrap();
    assert_eq!(broadenings(&r, &aug_decode("otu")), rules("(ao)[t](eu)→d"));
    assert_eq!(broadenings(&r, &aug_decode("otet")), rules("(ao)[t]e→d"));
    // Already matches, or the context would run off the word.
    assert!(broadenings(&r, &aug_decode("ate")).is_empty());
    assert!(broadenings(&r, &aug_decode("ot")).is_empty());
    // ^ must still hold.
    let r = HalfRule::decode("^a[t]→d").unwrap();
    assert_eq!(broadenings(&r, &aug_decode("ot")), rules("^(ao)[t]→d"));
    assert!(broadenings(&r, &aug_decode("xot")).is_empty());
  }

  #[test]
  fn broadening_fixes_a_word() {
    // e[x]→ks misses "ax"; broadening its context to (ae) fixes it, and
    // "ox", which is right already, gives no candidates.
    let d = dictionary(&[("ex", "eks"), ("ax", "aks"), ("ox", "oz")]);
    let mut pass = SecondPass::new(&d, rules("e[x]→ks\n[x]→z"));
    let before = pass.score();
    let stats = pass.broaden_sweep(true);
    assert_eq!(pass.rules, rules("(ae)[x]→ks\n[x]→z"));
    assert_eq!(stats.replaced, 1);
    assert!(stats.score_after < before);
  }

  #[test]
  fn removes_a_harmful_rule() {
    let d = dictionary(&[("cat", "kæt"), ("hat", "hæt")]);
    let mut pass = SecondPass::new(&d, rules("[c]→k\n[t]→d\n[a]→æ"));
    pass.sweep(true);
    assert_eq!(pass.rules, rules("[c]→k\n[a]→æ"));
  }

  #[test]
  fn pair_collapses_a_token() {
    // Neither half helps on its own if changed alone, but together the two
    // rules can become one.
    let d = dictionary(&[("sing", "sɪŋ"), ("ring", "rɪŋ"), ("sin", "sɪn")]);
    let mut pass = SecondPass::new(&d, rules("[i]→ɪ\n[ng]→{0}\n[{0}]→ŋ"));
    let singles = pass.sweep(true);
    assert_eq!(singles.replaced + singles.removed, 0);
    let changes = pass.pair_sweep(true);
    assert_eq!(changes.len(), 1);
    assert_eq!(pass.rules, rules("[i]→ɪ\n[ng]→ŋ"));
  }
  
  #[test]
  fn triple_collapses_a_chain() {
    // A chain of three rules can become one in a single triple move.
    let d = dictionary(&[("sing", "sɪŋ"), ("ring", "rɪŋ"), ("sin", "sɪn")]);
    let mut pass = SecondPass::new(&d, rules("[i]→ɪ\n[ng]→{0}\n[{0}]→{1}\n[{1}]→ŋ"));
    assert_eq!(tuples_from(&pass.rules, 1, 3), vec![vec![1, 2, 3]]);
    let change = pass.try_tuple(&[1, 2, 3]).unwrap();
    println!("{:?}", change);
    assert_eq!(pass.rules, rules("[i]→ɪ\n[ng]→ŋ"));
  }
  
  // The parallel tuple sweep must change exactly what the sequential one does.
  #[test]
  #[ignore]
  fn parallel_tuple_sweep_matches_sequential() {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(300);
    let hl = crate::high_level_substitutions2::HLSubstitutionList::decode(&std::fs::read_to_string("working/first-pass-300-30.txt").unwrap()).unwrap();
    let setup = || {
      let mut pass = SecondPass::new(&dictionary, crate::half_rules::flatten(&hl));
      pass.options.exact_scoring = true;
      pass.options.outputs_from_all_stages = true;
      pass.options.output_window = Some(0);
      pass.scale = 1.0;
      pass
    };
    let mut sequential = setup();
    let mut parallel = setup();
    let a = sequential.tuple_sweep(2, false);
    let b = parallel.tuple_sweep_parallel(2, 4, false);
    println!("{} changes", a.len());
    assert!(a.len() > 0);
    assert_eq!(a.len(), b.len());
    assert_eq!(sequential.rules, parallel.rules);
  }
  
  #[test]
  fn checkpoint_round_trip() {
    let d = dictionary(&[("cat", "kæt"), ("hat", "hæt"), ("sat", "sæt")]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cp.txt").to_str().unwrap().to_owned();
    let mut pass = SecondPass::new(&d, rules("[a]→{0}\n[c]→k\n[{0}]→ɑ"));
    pass.checkpoint_path = Some(path.clone());
    pass.scale = 0.5;
    pass.sweep(false);
    // The sweep changed the rules, so it saved them.
    let saved = SecondPass::load_checkpoint(&path).unwrap();
    assert_eq!(saved.rules, pass.rules);
    assert_eq!(saved.scale, Some(pass.scale));
    assert_eq!(saved.position, 0);
  }
  
  #[test]
  fn joint_scores_are_exact() {
    let d = dictionary(&[("ca", "ka"), ("ce", "se"), ("co", "ko"), ("ci", "si")]);
    let pass = SecondPass::new(&d, rules("[c]→{0}\n[{0}]→k\n[c]→k"));
    let options = SecondPassOptions { exact_scoring: true, outputs_from_all_stages: true, joint_tuples: true, ..Default::default() };
    let targets = compute_targets(&pass.rules, &pass.words);
    let es = JointEditSystem::new(&[0, 1], &pass.rules, &pass.words, &targets, options);
    let before = pass.score();
    let mut checked = 0;
    for w in 0 .. pass.words.len() {
      for c in es.find_improving_edits(w) {
        let weighted: f64 = (0 .. pass.words.len()).map(|v| {
          pass.words[v].frequency * (es.new_distance(&c.sub, v).unwrap() as f64 - es.baseline[v] as f64)
        }).sum::<f64>() + c.size_cost;
        let mut rules = pass.rules.clone();
        rules[0] = c.sub[0].clone();
        rules[1] = c.sub[1].clone();
        assert!((weighted - (global_score(&rules, &pass.words) - before)).abs() < 1e-9, "{:?}", c.sub);
        checked += 1;
      }
    }
    assert!(checked > 0);
  }
  
  #[test]
  fn joint_pair_keeps_both_rules() {
    // "c" before e or i should be s. Changing either of the first two rules
    // alone can't give that without breaking "ca" and "co"; changing both
    // together can.
    let d = dictionary(&[("ca", "ka"), ("ce", "se"), ("co", "ko"), ("ci", "si")]);
    let mut pass = SecondPass::new(&d, rules("[c]→{0}\n[{0}]→k\n[c]→k"));
    pass.options = SecondPassOptions { exact_scoring: true, outputs_from_all_stages: true, joint_tuples: true, ..Default::default() };
    let before = pass.score();
    let change = pass.try_tuple(&[0, 1]).unwrap();
    println!("{:?}", change);
    // One pair change fixes one of "ce" and "ci" (they gain the same), and
    // both rules stay.
    assert!(pass.score() < before - 0.9);
    assert_eq!(pass.rules.len(), 3);
    assert!(change.new.iter().all(|r| r.is_some()));
  }
  
  #[test]
  fn joint_pair_optimistic() {
    let d = dictionary(&[("ca", "ka"), ("ce", "se"), ("co", "ko"), ("ci", "si")]);
    let mut pass = SecondPass::new(&d, rules("[c]→{0}\n[{0}]→k\n[c]→k"));
    pass.options = SecondPassOptions { exact_scoring: true, outputs_from_all_stages: true, joint_tuples: true, optimistic_beam: true, ..Default::default() };
    let before = pass.score();
    let change = pass.try_tuple(&[0, 1]).unwrap();
    println!("{:?}", change);
    assert!(pass.score() < before - 0.9);
    assert_eq!(pass.rules.len(), 3);
  }
  
  // With joint tuples and no adaptation, the parallel tuple sweep must change
  // exactly what the sequential one does.
  #[test]
  #[ignore]
  fn parallel_joint_sweep_matches_sequential() {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(300);
    let hl = crate::high_level_substitutions2::HLSubstitutionList::decode(&std::fs::read_to_string("working/first-pass-300-30.txt").unwrap()).unwrap();
    let setup = || {
      let mut pass = SecondPass::new(&dictionary, crate::half_rules::flatten(&hl));
      pass.options.exact_scoring = true;
      pass.options.outputs_from_all_stages = true;
      pass.options.output_window = Some(0);
      pass.options.joint_tuples = true;
      pass.options.beam = 8;
      pass.scale = 1.0;
      pass
    };
    let mut sequential = setup();
    let mut parallel = setup();
    let a = sequential.tuple_sweep(2, false);
    let b = parallel.tuple_sweep_parallel(2, 4, false);
    println!("{} changes", a.len());
    assert_eq!(a.len(), b.len());
    assert_eq!(sequential.rules, parallel.rules);
  }
  
  #[test]
  fn align_boundaries_test() {
    use crate::glyphs::aug_decode;
    let same = |v: Vec<usize>| v.into_iter().map(|j| (j, j)).collect::<Vec<_>>();
    assert_eq!(align_boundaries(&aug_decode("cat"), &aug_decode("kæt")), same(vec![0, 1, 2, 3]));
    assert_eq!(align_boundaries(&aug_decode("{0}at"), &aug_decode("ϑæt")), same(vec![0, 1, 2, 3]));
    // "ng" lines up with the single ŋ. (When nothing but the first glyph
    // matches, as in "sing"/"sɪŋ", several alignments tie; the output window
    // absorbs that.)
    assert_eq!(align_boundaries(&aug_decode("sing"), &aug_decode("siŋ")), same(vec![0, 1, 2, 2, 3]));
    // Inserted glyphs: the boundary lines up with a range.
    assert_eq!(align_boundaries(&aug_decode("ab"), &aug_decode("abc")), vec![(0, 0), (1, 1), (2, 3)]);
    assert_eq!(align_boundaries(&aug_decode(""), &aug_decode("ab")), vec![(0, 2)]);
  }
  
  #[test]
  fn unreachable_targets_are_flagged() {
    // [s]$→{7} removes every final s, so no input can produce "kæts".
    let d = dictionary(&[("cats", "kæts"), ("dogs", "dɑgz")]);
    let r = rules("[s]$→{7}\n[{7}]→z");
    let pass = SecondPass::new(&d, r.clone());
    let targets = compute_targets(&pass.rules, &pass.words);
    assert_eq!(targets.reachable[0], vec![false, true]);
    assert_eq!(targets.reachable[1], vec![true, true]);
    assert_eq!(targets.reachable[2], vec![true, true]);
  }

  // A sweep over real first-pass rules must never make the global score
  // worse, and its bookkeeping must match a from-scratch recomputation.
  #[test]
  #[ignore]
  fn sweep_after_first_pass() {
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(300);
    let system = GaussianSystem { scale: 4.0 };

    let mut first = crate::first_pass::FirstPass::setup(&dictionary, crate::high_level_substitutions2::HLSubstitutionList { substitutions: vec![] });
    for _ in 0 .. 30 {
      first.find_next_rule(&system, false);
    }

    let mut pass = SecondPass::new(&dictionary, crate::half_rules::flatten(&first.rules));
    let stats = pass.sweep(true);
    println!("{:?}", stats);
    assert!(stats.score_after <= stats.score_before);
    assert!((stats.score_after - pass.score()).abs() < 1e-6);
  }
}
