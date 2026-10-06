// Grouping half-rules into as few OpenType lookups as possible.
//
// A half-rule alone in a lookup behaves exactly as HalfRule::apply. Several
// in one lookup behave differently: at each position the first one that
// matches wins (or, for a ^/$ ignore rule, blocks the rest), and the scan
// moves on. Fewer lookups should shape faster, so rules are merged
// empirically: going through the rules in order, each is appended to the
// current lookup if that doesn't make the dictionary's score worse, and
// otherwise starts a new lookup.

use rayon::prelude::*;

use crate::astarlike2::distance;
use crate::glyphs::AugGlyph;
use crate::half_rules::{HalfRule, ScoringWord, apply_all_copied};
use crate::substitutions2 as s2;

fn lookup_of(group: &[HalfRule]) -> s2::Lookup {
  s2::Lookup { substitutions: group.iter().flat_map(|r| r.low_level()).collect() }
}

// The kind of context-free substitution a rule can be written as, or None
// if it needs a chaining contextual lookup.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum PlainKind { Single, Multiple, Ligature }

pub fn plain_kind(r: &HalfRule) -> Option<PlainKind> {
  if !r.pre.is_empty() || !r.post.is_empty() || r.at_start || r.at_end || r.not_at_start || r.not_at_end {
    None
  }
  else if r.at.len() > 1 {
    Some(PlainKind::Ligature)
  }
  else if r.out.len() > 1 {
    Some(PlainKind::Multiple)
  }
  else {
    Some(PlainKind::Single)
  }
}

// A group of rules in the order a plain lookup made from it would try them.
// If they're all context-free and of one kind, a rule with the same key as
// an earlier one (which could never apply) is dropped, and feaLib lists
// ligatures longest first (keeping the order among equal lengths).
// Otherwise the group is unchanged.
pub fn plain_order(group: &[HalfRule]) -> Vec<HalfRule> {
  let kind = plain_kind(&group[0]);
  if kind.is_none() || group.iter().any(|r| plain_kind(r) != kind) {
    return group.to_vec();
  }
  let mut res: Vec<HalfRule> = vec![];
  for r in group {
    if !res.iter().any(|q| q.at == r.at) {
      res.push(r.clone());
    }
  }
  res.sort_by_key(|r| std::cmp::Reverse(r.at.len()));
  res
}

// The substitutions for a font: one lookup per group.
pub fn low_level(groups: &[Vec<HalfRule>]) -> s2::SubstitutionList {
  s2::SubstitutionList { lookups: groups.iter().map(|g| lookup_of(g)).collect() }
}

// Frequency-weighted distance of each word's output through the grouped
// lookups (without the rules' size cost, which grouping doesn't change).
pub fn grouped_distance(groups: &[Vec<HalfRule>], words: &[ScoringWord]) -> f64 {
  let slist = low_level(groups);
  words.par_iter().map(|w| {
    let mut output = w.spelling.clone();
    s2::apply_all(&mut output, &slist);
    w.frequency * (distance(&output, &w.pronunciation) as f64)
  }).sum()
}

// Merges consecutive rules into shared lookups wherever that doesn't make
// the weighted distance worse. Returns the groups, in order.
//
// For each word it keeps the state before the current lookup and after it.
// Trying the next rule then means comparing the lookup with the rule
// appended against the lookup followed by the rule on its own; only words
// where those differ need the remaining rules run and their distance
// rechecked.
pub fn merge(rules: &[HalfRule], words: &[ScoringWord]) -> Vec<Vec<HalfRule>> {
  merge_with(rules, words, false)
}

// With `plain`, only rules of the same plain kind (see plain_kind) share a
// lookup, so every context-free lookup can be written as a plain one, and
// each lookup is simulated, and returned, in plain_order.
pub fn merge_with(rules: &[HalfRule], words: &[ScoringWord], plain: bool) -> Vec<Vec<HalfRule>> {
  let n = words.len();
  let mut groups: Vec<Vec<HalfRule>> = vec![];
  if rules.is_empty() {
    return groups;
  }

  let mut group: Vec<HalfRule> = vec![rules[0].clone()];
  let mut before: Vec<Vec<AugGlyph>> = words.iter().map(|w| w.spelling.clone()).collect();
  let mut after: Vec<Vec<AugGlyph>> = before.iter().map(|s| {
    let mut s = s.clone();
    rules[0].apply(&mut s);
    s
  }).collect();
  let mut distances: Vec<u32> = words.par_iter().map(|w| distance(&apply_all_copied(rules, &w.spelling), &w.pronunciation)).collect();

  for idx in 1 .. rules.len() {
    let rule = &rules[idx];
    let rest = &rules[idx + 1 ..];
    if plain && plain_kind(rule) != plain_kind(&group[0]) {
      groups.push(std::mem::replace(&mut group, vec![rule.clone()]));
      before = std::mem::take(&mut after);
      after = before.par_iter().map(|s| {
        let mut s = s.clone();
        rule.apply(&mut s);
        s
      }).collect();
      continue;
    }
    let mut trial = group.clone();
    trial.push(rule.clone());
    let merged_lookup = if plain { lookup_of(&plain_order(&trial)) } else { lookup_of(&trial) };

    // Per word: (state after the separate rule, state after the merged
    // lookup, new distance if they differ).
    let outcomes: Vec<(Vec<AugGlyph>, Vec<AugGlyph>, Option<u32>)> = (0 .. n).into_par_iter().map(|w| {
      let mut separate = after[w].clone();
      rule.apply(&mut separate);
      let mut merged = before[w].clone();
      s2::apply_lookup(&mut merged, &merged_lookup);
      let new_distance = if merged != separate {
        Some(distance(&apply_all_copied(rest, &merged), &words[w].pronunciation))
      }
      else {
        None
      };
      (separate, merged, new_distance)
    }).collect();

    let change: f64 = outcomes.iter().enumerate().map(|(w, (_, _, d))| match d {
      Some(d) => words[w].frequency * (*d as f64 - distances[w] as f64),
      None => 0.0
    }).sum();

    if change <= 1e-12 {
      group = trial;
      for (w, (_, merged, d)) in outcomes.into_iter().enumerate() {
        after[w] = merged;
        if let Some(d) = d {
          distances[w] = d;
        }
      }
    }
    else {
      groups.push(std::mem::replace(&mut group, vec![rule.clone()]));
      before = std::mem::take(&mut after);
      after = outcomes.into_iter().map(|(separate, _, _)| separate).collect();
    }
  }
  groups.push(group);
  if plain {
    groups.iter().map(|g| plain_order(g)).collect()
  }
  else {
    groups
  }
}

// The glyphs a rule reads (its key and context) and the ones it changes (its
// key and output).
fn reads(r: &HalfRule) -> impl Iterator<Item = &AugGlyph> {
  r.pre.iter().flatten().chain(r.at.iter()).chain(r.post.iter().flatten())
}

fn writes(r: &HalfRule) -> impl Iterator<Item = &AugGlyph> {
  r.at.iter().chain(r.out.iter())
}

// Whether two rules can't affect each other in either order: neither reads a
// glyph the other changes. (Sufficient, not necessary; used to pick which
// moves are worth checking.)
fn independent(a: &HalfRule, b: &HalfRule) -> bool {
  let (aw, bw): (Vec<&AugGlyph>, Vec<&AugGlyph>) = (writes(a).collect(), writes(b).collect());
  !reads(a).any(|g| bw.contains(&g)) && !reads(b).any(|g| aw.contains(&g))
}

// Like merge, but when the next rule can't join the current lookup, later
// rules may be moved up into it. A move is tried only for a rule independent
// of every rule it would move past (and within `window` rules of the
// lookup's end), and kept if the dictionary's weighted distance doesn't get
// worse, with everything else in its original order.
pub fn merge_reordering(rules: &[HalfRule], words: &[ScoringWord], window: usize) -> Vec<Vec<HalfRule>> {
  let mut remaining: Vec<HalfRule> = rules.to_vec();
  let mut groups: Vec<Vec<HalfRule>> = vec![];
  // Each word's state before the current lookup.
  let mut before: Vec<Vec<AugGlyph>> = words.iter().map(|w| w.spelling.clone()).collect();
  
  // Weighted distance with `group` as the current lookup, followed by `rest`
  // one rule at a time.
  let score = |before: &[Vec<AugGlyph>], group: &[HalfRule], rest: &[HalfRule]| -> f64 {
    let lookup = lookup_of(group);
    before.par_iter().zip(words.par_iter()).map(|(state, w)| {
      let mut s = state.clone();
      s2::apply_lookup(&mut s, &lookup);
      w.frequency * (distance(&apply_all_copied(rest, &s), &w.pronunciation) as f64)
    }).sum()
  };
  
  while !remaining.is_empty() {
    let mut group = vec![remaining.remove(0)];
    let mut current = score(&before, &group, &remaining);
    
    'grow: loop {
      for j in 0 .. remaining.len().min(window) {
        let candidate = &remaining[j];
        if !remaining[.. j].iter().all(|q| independent(q, candidate)) {
          continue;
        }
        let mut trial = group.clone();
        trial.push(candidate.clone());
        let mut rest = remaining.clone();
        rest.remove(j);
        let trial_score = score(&before, &trial, &rest);
        if trial_score <= current + 1e-12 {
          group = trial;
          remaining = rest;
          current = trial_score;
          continue 'grow;
        }
      }
      break;
    }
    
    let lookup = lookup_of(&group);
    before.par_iter_mut().for_each(|s| s2::apply_lookup(s, &lookup));
    groups.push(group);
  }
  groups
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::half_rules::flatten;
  use crate::high_level_substitutions2::HLSubstitutionList;

  fn words(n: usize) -> Vec<ScoringWord> {
    let dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.iter().take(n).map(|w| ScoringWord {
      spelling: w.spelling.iter().map(|g| AugGlyph::Real(*g)).collect(),
      pronunciation: w.pronunciation.iter().map(|g| AugGlyph::Real(*g)).collect(),
      frequency: w.frequency
    }).collect()
  }

  // Harfbuzz must shape merged lookups exactly as the simulator does. Uses
  // the 300-word, 100-rule first-pass rules, whose tokens fit in the test
  // font's syn0..syn99.
  #[test]
  #[ignore]
  fn merged_lookups_match_harfbuzz() {
    let hl = HLSubstitutionList::decode(&std::fs::read_to_string("working/first-pass-300-100-scale2.txt").unwrap()).unwrap();
    let rules = flatten(&hl);
    let words = words(300);
    let groups = merge(&rules, &words);
    println!("{} rules in {} lookups", rules.len(), groups.len());
    let slist = low_level(&groups);
    let texts: Vec<Vec<AugGlyph>> = words.iter().map(|w| w.spelling.clone()).collect();
    let shaped = crate::hbshape::apply_many_using_hbshape(&slist, &texts).unwrap();
    assert_eq!(shaped.len(), texts.len());
    for (text, by_hbshape) in texts.iter().zip(shaped.iter()) {
      let mut by_sim = text.clone();
      s2::apply_all(&mut by_sim, &slist);
      assert_eq!(crate::glyphs::aug_encode(by_hbshape), crate::glyphs::aug_encode(&by_sim), "{}", crate::glyphs::aug_encode(text));
    }
  }
  
  // Reordering must not make the score worse either, and shouldn't need
  // more lookups than merging in order.
  #[test]
  fn reordering_set_1() {
    let rules = flatten(&HLSubstitutionList::set_1());
    let words = words(1000);
    let separate: Vec<Vec<HalfRule>> = rules.iter().map(|r| vec![r.clone()]).collect();
    let before = grouped_distance(&separate, &words);
    let in_order = merge(&rules, &words);
    let reordered = merge_reordering(&rules, &words, 100);
    let after = grouped_distance(&reordered, &words);
    println!("{} rules: {} lookups in order, {} reordered; distance {:.4} -> {:.4}", rules.len(), in_order.len(), reordered.len(), before, after);
    assert!(after <= before + 1e-9);
    assert!(reordered.len() <= in_order.len());
    assert_eq!(reordered.iter().map(|g| g.len()).sum::<usize>(), rules.len());
  }
  
  // Merging must not make the score worse, and should merge a lot.
  #[test]
  fn merging_set_1() {
    let rules = flatten(&HLSubstitutionList::set_1());
    let words = words(3000);
    let separate: Vec<Vec<HalfRule>> = rules.iter().map(|r| vec![r.clone()]).collect();
    let before = grouped_distance(&separate, &words);
    let groups = merge(&rules, &words);
    let after = grouped_distance(&groups, &words);
    println!("{} rules in {} lookups; distance {:.4} -> {:.4}", rules.len(), groups.len(), before, after);
    assert!(after <= before + 1e-9);
    assert!(groups.len() < rules.len() / 2);
    assert_eq!(groups.iter().map(|g| g.len()).sum::<usize>(), rules.len());
  }
}
