
use crate::dictionary::Dictionary;
use crate::glyphs::{Glyph, AugGlyph::{Real, Synthetic}};
use crate::high_level_substitutions::{HLSubstitution, HLSubstitutionList, hl_to_ll_with_new, hl_to_ll};
use crate::substitutions2::{apply_all, apply_all_with_new};
use keyed_priority_queue::KeyedPriorityQueue;
use levenshtein_diff as levenshtein;
use float_ord::FloatOrd;
use std::collections::HashMap;
use rayon::prelude::*;
use noisy_float::prelude::*;

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

fn apply_copied_with_new(v: &Vec<Glyph>, hl_slist: &HLSubstitutionList, new_amount: usize) -> Option<Vec<Glyph>> {
  let (slist, new_amount) = hl_to_ll_with_new(hl_slist, new_amount).unwrap();
  let mut working = v.iter().map(|g| Real(g.clone())).collect();
  if apply_all_with_new(&mut working, &slist, new_amount) {
    Some(working.into_iter().map(|g| match g {
      Real(g) => g,
      Synthetic(_) => panic!("Leftover synthetic glyphs")
    }).collect())
  }
  else {
    None
  }
}

pub struct SubWithImprovement {
  pub sub: HLSubstitution,
  pub improvement: u32
}

fn is_letter(g: &Glyph) -> bool {
  use Glyph::*;
  match g {
    A | B | C | D | E | F | G | H | I | J | K | L | M | N | O | P | Q | R | S | T | U | V | W | X | Y | Z => true,
    _ => false
  }
}

pub fn find_improving_edits(word: &Vec<Glyph>, pronunciation: &Vec<Glyph>, init_hl_slist: &HLSubstitutionList) -> Vec<SubWithImprovement> {
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
  
  (0 .. word.len()).into_par_iter().map(|k1| {
    let mut working_hl_slist = working_hl_slist.clone();
    
    let mut res = vec![];
  
    for k2 in (k1+1) .. (word.len()+1) {
      let key_size = k2 - k1;
      for s1 in 0 .. key_size {
        for s2 in (s1+1) .. (key_size+1) {
          for sc1 in 0 .. pronunciation.len() {
            for sc2 in (sc1+1) .. (pronunciation.len()+1) {
              let can_be_at_start = k1 == 0 || !is_letter(&word[k1-1]);
              let can_be_at_end = k2 == word.len() || !is_letter(&word[k2]);
              
              for &at_start in if can_be_at_start {[true, false].iter()} else {[false].iter()} {
                for &at_end in if can_be_at_end {[true, false].iter()} else {[false].iter()} {
                  let hlsub = HLSubstitution {
                    key: word[k1 .. k2].to_vec(),
                    sub_start: s1,
                    sub_end: s2,
                    at_start,
                    at_end,
                    sub_content: pronunciation[sc1 .. sc2].to_vec()
                  };
                  working_hl_slist.substitutions[0] = hlsub;
                  if let Some(new_transformed) = apply_copied_with_new(word, &working_hl_slist, 1) {
                    let new_distance = distance(&new_transformed, pronunciation);
                    if new_distance < base_distance {
                      res.push(SubWithImprovement {
                        sub: working_hl_slist.substitutions[0].clone(),
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

pub trait Table<Estimate: Clone, Estimator: Clone> {
  fn introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize,
    change_at_introducing_i: i32,
    edit: &HLSubstitution
  ) -> Estimator;
  
  fn estimate_introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize
  ) -> Estimate;
  
  fn update_edit(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
    introduced_i: usize,
    updated_i: usize,
    change_at_updated_i: i32,
    prev_estimate: Estimator
  ) -> Estimator;
}

pub trait System<T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone> {
  fn build_table(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>
  ) -> T;
  
  fn calc_best_possible(&self, estimate: &Estimate) -> R64;
  fn calc_worst_possible(&self, estimate: &Estimate) -> R64;
  
  fn calc_estimate(&self, estimator: &Estimator) -> Estimate;
}

pub enum Outcome {
  FoundImprovement(HLSubstitution, f64),
  FailedToFindImprovement(HLSubstitution, f64, f64, f64)
}

#[derive(Debug, Clone)]
pub struct WorkingEntry<E> {
  estimate: E,
  best_possible: f64,
  worst_possible: f64,
  
  // This is an index into the introducing vector,
  // which is ordered by weighted badness.
  introducing_index: usize,
  
  // This is an index into the exploring vector,
  // which is ordered by frequency.
  next_to_explore_index: usize
}

pub struct ReferenceData<'d, 'h, T> {
  dictionary: &'d Dictionary,
  init_hl_slist: &'h HLSubstitutionList,
  
  n: usize,
  current_distances: Vec<u32>,
  
  table: T,
  
  introducing_order: Vec<usize>,
  introducing_order_rev: Vec<usize>,
  
  frequencies_in_introducing_order: Vec<f64>,
  current_distances_in_introducing_order: Vec<u32>
}

fn edit_size_cost(edit: &HLSubstitution) -> f64 {
  let size = edit.key.len() + (if edit.at_start { 1 } else { 0 }) + (if edit.at_end { 1 } else { 0 });
  (size as f64) * 0.001
}

pub fn init_ref_data<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone, S: System<T, Estimate, Estimator>>(sys: &S, dictionary: &'d Dictionary, init_hl_slist: &'h HLSubstitutionList) -> ReferenceData<'d, 'h, T> {
  let n = dictionary.words.len();

  let current_distances: Vec<u32> = dictionary.words.iter().map(|w| {
    distance(&apply_copied(&w.spelling, init_hl_slist), &w.pronunciation)
  }).collect();
  
  let mut introducing_order: Vec<usize> = (0 .. n).collect();
  introducing_order.sort_by_key(|&i| FloatOrd(-dictionary.words[i].frequency * (current_distances[i] as f64)));
  let introducing_order = introducing_order;
  
  let mut introducing_order_rev = vec![0; n];
  for i in 0 .. n {
    let j = introducing_order[i];
    introducing_order_rev[j] = i;
  }

  let frequencies_in_introducing_order = (0 .. n).map(|i| dictionary.words[introducing_order[i]].frequency).collect();
  
  let current_distances_in_introducing_order = (0 .. n).map(|i| current_distances[introducing_order[i]]).collect();
  
  let table = sys.build_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order);
  
  ReferenceData {
    dictionary,
    init_hl_slist,
    
    n,
    current_distances,
    table,
    
    introducing_order,
    introducing_order_rev,
    
    frequencies_in_introducing_order,
    current_distances_in_introducing_order
  }
}

pub struct WorkingData<Estimator> {
  working_table: HashMap<HLSubstitution, WorkingEntry<Estimator>>,
  best_possible: KeyedPriorityQueue<HLSubstitution, FloatOrd<f64>>,
  best_possible_rev: KeyedPriorityQueue<HLSubstitution, FloatOrd<f64>>,
  
  introducing_working_index: usize
}

fn improving_edits_at_i<'d, 'h, T>(r: &ReferenceData<'d, 'h, T>, i: usize) -> Vec<SubWithImprovement> {
  let j = r.introducing_order[i];
  let w = &r.dictionary.words[j];
  find_improving_edits(&w.spelling, &w.pronunciation, r.init_hl_slist)
}

fn fmt_word(w: &Vec<Glyph>) -> String {
  w.iter().map(|g| g.char()).collect::<Vec<String>>().join("")
}

pub fn init_working_data<'d, 'h, T, E>(_r: &ReferenceData<'d, 'h, T>) -> WorkingData<E> {
  let working_table: HashMap<HLSubstitution, WorkingEntry<E>> = HashMap::new();
  let best_possible: KeyedPriorityQueue<HLSubstitution, FloatOrd<f64>> = KeyedPriorityQueue::new();
    
  // This stores working items in *reverse* order by best_possible. Items that cannot be as good
  // as the worst_possible of the top of the best_possible queue should be deleted.
  let best_possible_rev: KeyedPriorityQueue<HLSubstitution, FloatOrd<f64>> = KeyedPriorityQueue::new();
  
  WorkingData {
    working_table,
    best_possible,
    best_possible_rev,
    
    introducing_working_index: 0
  }
}

fn estimate_from_introducing_iterator<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone>(r: &ReferenceData<'d, 'h, T>, w: &mut WorkingData<Estimator>, _debug: bool) -> Option<Estimate> {
  if w.introducing_working_index < r.n {
    let i = w.introducing_working_index;
    Some(r.table.estimate_introduce(&r.frequencies_in_introducing_order, i))
  }
  else {
    None
  }
}

fn introduce<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone, S: System<T, Estimate, Estimator>>(sys: &S, r: &ReferenceData<'d, 'h, T>, w: &mut WorkingData<Estimator>, debug: bool) {
  // This occurs if either: (1) there is nothing in the heap, or (2) the
  // heap top's best_possible is not as good as best_possible_from_introducing_iterator.
  
  // Add it to the working table.
  // Note that we need to consider the introducing_index in initializing
  // best_possible and worst_possible.
  
  if w.introducing_working_index >= r.n {
    return;
  }
  
  let introducing_improving_edits = improving_edits_at_i(r, w.introducing_working_index);
   
  let i = w.introducing_working_index;
  let j = r.introducing_order[i];
  
  for edit in introducing_improving_edits {
    if !w.working_table.contains_key(&edit.sub) {
      let improvement = edit.improvement;
      let change = -(improvement as i32);
      
      if debug {
        println!("introduce {:?} (word {})", edit.sub, fmt_word(&r.dictionary.words[j].spelling));
        println!("  improvement: from {} to {} ({})", r.current_distances[j], r.current_distances[j]-edit.improvement, edit.improvement);
      }
      
      let init_estimate: Estimator = r.table.introduce(&r.frequencies_in_introducing_order, i, change, &edit.sub);
      let e = sys.calc_estimate(&init_estimate);
      let size_cost = edit_size_cost(&edit.sub);
      let best_possible = sys.calc_best_possible(&e).raw() + size_cost;
      let worst_possible = sys.calc_worst_possible(&e).raw() + size_cost;
      
      let entry = WorkingEntry {
        estimate: init_estimate,
        best_possible,
        worst_possible,
        introducing_index: w.introducing_working_index,
        next_to_explore_index: 0,
      };
      
      w.working_table.insert(edit.sub.clone(), entry);
      w.best_possible.push(edit.sub.clone(), FloatOrd(-best_possible));
      w.best_possible_rev.push(edit.sub.clone(), FloatOrd(best_possible));
    }
    else {
      if debug { println!("We've seen {:?} before, skipping it.", edit.improvement); }
    }
  }
  
  w.introducing_working_index += 1;
}

fn cull_working_table<'d, 'h, E>(w: &mut WorkingData<E>, debug: bool) {
  // Use best_possible_rev to delete working table entries where the best possible is not
  // better than the worst_possible if the top of best_possible.
  if debug { println!("cull_working_table"); }
  
  let Some(best_top) = w.best_possible.peek() else { return };
  let best_worst = w.working_table.get(best_top.0).unwrap().worst_possible;
  
  if debug { println!("  best_worst = {} ({:?})", best_worst, best_top.0); }
  
  loop {
    if w.best_possible_rev.len() <= 1 {
      if debug { println!("  Only one edit, stopping cull."); }
      return;
    }
    
    let worst_best = w.best_possible_rev.peek().unwrap().1.0;
    
    if worst_best >= best_worst {
      let sub = w.best_possible_rev.peek().unwrap().0;
      if debug { println!("  Removing {:?} because {} is not better than {}", sub, worst_best, best_worst); }
      w.best_possible.remove(sub);
      w.working_table.remove(sub);
      w.best_possible_rev.pop();
    }
    else {
      if debug { println!("  Stopping cull because {} could beat {}", worst_best, best_worst); }
      return;
    }
  }
}

fn new_distance_if_new(spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>, init_hl_slist: &HLSubstitutionList, sub: &HLSubstitution) -> Option<u32> {
  let mut working_hl_slist = init_hl_slist.clone();
  working_hl_slist.substitutions.insert(0, sub.clone());
  if let Some(new_transformed) = apply_copied_with_new(spelling, &working_hl_slist, 1) {
    let new_distance = distance(&new_transformed, pronunciation);
    Some(new_distance)
  }
  else {
    None
  }
}

fn change_with_new(spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>, init_hl_slist: &HLSubstitutionList, sub: &HLSubstitution, orig_dist: u32) -> i32 {
  match new_distance_if_new(spelling, pronunciation, init_hl_slist, sub) {
    Some(new_dist) => (new_dist as i32) - (orig_dist as i32),
    None => 0
  }
}

fn calc_change(spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>, init_hl_slist: &HLSubstitutionList, sub: &HLSubstitution, orig_dist: u32) -> i32 {
  change_with_new(spelling, pronunciation, init_hl_slist, sub, orig_dist)
}

fn total_unsized_cost(dictionary: &Dictionary, hl_slist: &HLSubstitutionList) -> f64 {
  let distance_cost: f64 = dictionary.words.iter().map(|w| {
    (distance(&apply_copied(&w.spelling, hl_slist), &w.pronunciation) as f64) * w.frequency
  }).sum();
  
  distance_cost
}

fn produce_outcome<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone>(r: &ReferenceData<'d, 'h, T>, best_possible_sub: HLSubstitution, best_possible: f64) -> Outcome {
  let before_cost = total_unsized_cost(&r.dictionary, &r.init_hl_slist);

  let mut new_hl_slist = r.init_hl_slist.clone();
  new_hl_slist.substitutions.insert(0, best_possible_sub.clone());
  
  let after_cost = total_unsized_cost(&r.dictionary, &new_hl_slist);
  
  if after_cost < before_cost {
    Outcome::FoundImprovement(best_possible_sub, after_cost - before_cost)
  }
  else {
    Outcome::FailedToFindImprovement(best_possible_sub, best_possible, before_cost, after_cost)
  }
}

fn check_for_winner<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone>(r: &ReferenceData<'d, 'h, T>, w: &WorkingData<Estimator>, best_possible_from_introducing_iterator: Option<f64>, debug: bool) -> Option<Outcome> {
  let best_possible_sub = w.best_possible.peek().unwrap().0.clone();
  
  if debug { println!("advancing {:?}", best_possible_sub); }
  
  let working_table_len = w.working_table.len();
  let working = w.working_table.get(&best_possible_sub).unwrap();
  
  if working_table_len == 1 && best_possible_from_introducing_iterator.map_or_else(|| true, |b| working.worst_possible <= b) {
    if debug { println!("Found the winner by priority: {:?} {}/{} {} {} {:?}", best_possible_sub, working.next_to_explore_index, r.n, working.best_possible, working.worst_possible, best_possible_from_introducing_iterator); }
    Some(produce_outcome(r, best_possible_sub, working.best_possible))
  }
  else {
    let j = working.next_to_explore_index;
    
    if j >= r.n {
      // This case can really only happen due to rounding error.
      // At this point, we know:
      // * Its best_possible is equal to its worst_possible
      // * Its best_possible is at least as good as any other best_possible in the table
      // * Its best_possible is at least as good as the best possible from introducing iterator
      if debug { println!("Found the winner by default: {:?}", best_possible_sub); }
      Some(produce_outcome(r, best_possible_sub, working.best_possible))
    }
    else {
      None
    }
  }
}

struct WorkIntermediate<E> {
  estimate: E,
  next_to_explore_index: usize
}

fn advance_many<'d, 'h, T: Table<Estimate, Estimator>, Estimate: Clone, Estimator: Clone>(r: &ReferenceData<'d, 'h, T>, sub: &HLSubstitution, working: &WorkingEntry<Estimator>, num_to_advance: usize, debug: bool) -> WorkIntermediate<Estimator> {
  let mut j = working.next_to_explore_index;
  let mut k = 0;
  let mut estimate = working.estimate.clone();
  
  while k < num_to_advance && j < r.n {
    let i = r.introducing_order_rev[j];
    
    if debug { println!("  word = {}", fmt_word(&r.dictionary.words[j].spelling)); }
    
    if i == working.introducing_index {
      if debug { println!("  {} {}/{} was seen before, moving on.", fmt_word(&r.dictionary.words[j].spelling), j, i); }
      // Nothing to do, we've already seen this one
    }
    else {
      let word = &r.dictionary.words[j];
      
      let change = calc_change(&word.spelling, &word.pronunciation, r.init_hl_slist, sub, r.current_distances[j]);
      
      if debug {
        println!("  old distance = {}", r.current_distances[j]);
        println!("  change = {}", change);
      }
      
      estimate = r.table.update_edit(
        &r.frequencies_in_introducing_order,
        &r.current_distances_in_introducing_order,
        working.introducing_index,
        i,
        change,
        estimate
      );
    }
    
    k += 1;
    j += 1;
  }
  
  WorkIntermediate {
    estimate,
    next_to_explore_index: j
  }
}

struct AdvancingWork<Estimator> {
  edit: HLSubstitution,
  working: WorkingEntry<Estimator>
}

struct AdvancingResult<Estimator> {
  edit: HLSubstitution,
  estimate: Estimator,
  best_possible: f64,
  worst_possible: f64,
  next_to_explore_index: usize
}

fn do_work<'d, 'h, T: Table<Estimate, Estimator> + Send + Sync, Estimate: Clone + Send + Sync, Estimator: Clone + Send + Sync, S: System<T, Estimate, Estimator> + Send + Sync>(sys: &S, r: &ReferenceData<'d, 'h, T>, w: &mut WorkingData<Estimator>, _best_possible_from_introducing_iterator: Option<f64>, debug: bool) {
  // To take advantage of multiple CPU cores, we process edits in chunks
  let edit_chunk_size = 256;
  let steps_chunk_size = 256;
  
  // Go explore the next word and update best_possible and worst_possible accordingly. Note
  // that in updating best_possible, we need to consider this entry's introducing_index,
  // since that will tell us which words it could conceivably improve.
  
  let mut edit_chunk: Vec<AdvancingWork<Estimator>> = vec![];
  for _ in 0 .. edit_chunk_size {
    if w.best_possible.is_empty() {
      break;
    }
      
    let sub = w.best_possible.pop().unwrap().0;
    let working = (*w.working_table.get(&sub).unwrap()).clone();
    edit_chunk.push(AdvancingWork {
      edit: sub,
      working
    });
  }
  
  let result_chunk: Vec<AdvancingResult<Estimator>> = edit_chunk.into_par_iter().map(|work| {
    let best_possible_sub = work.edit;
    let working = work.working;
    
    if debug { println!("advancing {:?}", best_possible_sub); }
    
    let intermediate = advance_many(r, &best_possible_sub, &working, steps_chunk_size, debug);
    
    let e = sys.calc_estimate(&intermediate.estimate);
    let size_cost = edit_size_cost(&best_possible_sub);
    let best_possible = sys.calc_best_possible(&e).raw() + size_cost;
    let worst_possible = sys.calc_worst_possible(&e).raw() + size_cost;
    
    AdvancingResult {
      edit: best_possible_sub,
      estimate: intermediate.estimate,
      best_possible,
      worst_possible,
      next_to_explore_index: intermediate.next_to_explore_index
    }
  }).collect();
  
  for result in result_chunk {
    w.best_possible_rev.set_priority(&result.edit, FloatOrd(result.best_possible)).unwrap();
    w.best_possible.push(result.edit.clone(), FloatOrd(-result.best_possible));
    
    let working = w.working_table.get_mut(&result.edit).unwrap();
    working.estimate = result.estimate;
    working.best_possible = result.best_possible;
    working.worst_possible = result.worst_possible;
    working.next_to_explore_index = result.next_to_explore_index;
  }
}

fn dump_state<'d, 'h, T, E>(r: &ReferenceData<'d, 'h, T>, w: &mut WorkingData<E>) {
  for j in 0 .. (r.dictionary.words.len()+1) {
    for (edit, w) in w.working_table.iter() {
      if w.next_to_explore_index == j {
        println!("          {:>10} bp={:>4.1} wp={:>4.1}", format!("{:?}", edit), w.best_possible, w.worst_possible);
      }
    }
    if j < r.dictionary.words.len() {
      println!("  {:>4} f={:>4.2} d={:>3}", fmt_word(&r.dictionary.words[j].spelling), r.dictionary.words[j].frequency, r.current_distances[j]);
    }
  }
}

pub fn step<'d, 'h, T: Table<Estimate, Estimator> + Send + Sync, Estimate: Clone + Send + Sync, Estimator: Clone + Send + Sync, S: System<T, Estimate, Estimator> + Send + Sync>(sys: &S, r: &ReferenceData<'d, 'h, T>, w: &mut WorkingData<Estimator>, debug: bool) -> Option<Outcome> {
  if debug { println!("step"); }
  
  if debug { dump_state(r, w); }
  
  if w.working_table.is_empty() {
    if debug { println!("Working table is empty."); }
    introduce(sys, r, w, debug);
    None
  }
  else {
    // Cull the working table by starting with the top of best_possible_rev.
    // This means that once the working table is down to a single element, and that element's
    // worst_possible is better than best_possible_from_introducing_iterator, the algorithm
    // may terminate.
    cull_working_table(w, debug);
    
    let estimate_from_introducing_iterator = estimate_from_introducing_iterator(r, w, debug);
    let best_possible_from_introducing_iterator = estimate_from_introducing_iterator.map(|e| sys.calc_best_possible(&e).raw());
    
    let best_possible_top = w.best_possible.peek().unwrap();
    let best_possible_from_working_table = -best_possible_top.1.0;
    
    if debug { println!("best_possible_from_working_table = {}", best_possible_from_working_table); }
    
    let intro_iter_is_better = best_possible_from_introducing_iterator.map_or_else(|| false, |b| b < best_possible_from_working_table);
    
    if debug { println!("Best possible:"); }
    if debug { println!("  from new ones: {:?} (word {}/{}/{}) {}",
      best_possible_from_introducing_iterator,
      if w.introducing_working_index < r.n { fmt_word(&r.dictionary.words[r.introducing_order_rev[w.introducing_working_index]].spelling) } else { "NA".to_owned() },
      w.introducing_working_index,
      if w.introducing_working_index < r.n { r.introducing_order_rev[w.introducing_working_index] } else { r.n },
      if intro_iter_is_better {"*"} else {""}
    ); }
    if debug { println!("  from old ones: {:.2} ({:?}) {}", best_possible_from_working_table, best_possible_top.0, if intro_iter_is_better {""} else {"*"}); }
    
    if intro_iter_is_better {
      introduce(sys, r, w, debug);
      None
    }
    else {
      if let Some(res) = check_for_winner(r, w, best_possible_from_introducing_iterator, debug) {
        Some(res)
      }
      else {
        do_work(sys, r, w, best_possible_from_introducing_iterator, debug);
        None
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;
  
  #[derive(Clone)]
  struct Test1Estimator {
    path: Vec<Test1Estimate>,
    index: usize
  }
  
  #[derive(Clone, Copy)]
  struct Test1Estimate {
    best_possible: f64,
    worst_possible: f64
  }
  
  #[derive(Clone)]
  struct Test1Word {
    specific_paths: HashMap<HLSubstitution, Vec<Test1Estimate>>,
    default_path: Vec<Test1Estimate>
  }
  
  struct Test1Table {
    complete_table: Vec<Test1Word>
  }
  
  impl Table<Test1Estimate, Test1Estimator> for Test1Table {
    fn introduce(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      introducing_i: usize,
      _change_at_introducing_i: i32,
      edit: &HLSubstitution
    ) -> Test1Estimator {
      let word = &self.complete_table[introducing_i];
      let path = match word.specific_paths.get(edit) {
        None => word.default_path.clone(),
        Some(path) => path.clone()
      };
      Test1Estimator {
        path,
        index: introducing_i
      }
    }
    
    fn estimate_introduce(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      introducing_i: usize
    ) -> Test1Estimate {
      self.complete_table[introducing_i].default_path[0]
    }
    
    fn update_edit(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      _current_distances_in_introducing_order: &Vec<u32>,
      _introduced_i: usize,
      updated_i: usize,
      _change_at_updated_i: i32,
      prev_estimate: Test1Estimator
    ) -> Test1Estimator {
      Test1Estimator {
        path: prev_estimate.path,
        index: updated_i
      }
    }
  }

  struct Test1System {
    complete_table: Vec<Test1Word>
  }
  
  impl System<Test1Table, Test1Estimate, Test1Estimator> for Test1System {
    fn build_table(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      _current_distances_in_introducing_order: &Vec<u32>
    ) -> Test1Table {
      Test1Table {
        complete_table: self.complete_table.clone()
      }
    }
    
    fn calc_estimate(&self, estimator: &Test1Estimator) -> Test1Estimate {
      estimator.path[estimator.index]
    }
    
    fn calc_best_possible(&self, e: &Test1Estimate) -> R64 { r64(e.best_possible) }
    fn calc_worst_possible(&self, e: &Test1Estimate) -> R64 { r64(e.worst_possible) }
  }
  
  #[test]
  fn astarlike_test_1() {
    use crate::glyphs::Glyph::*;
    use crate::dictionary::DictionaryWord;
    
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![A, T],
          pronunciation: vec![Ae, T],
          frequency: 1.0
        },
        DictionaryWord {
          spelling: vec![A, S],
          pronunciation: vec![Ae, Z],
          frequency: 0.5
        }
      ]
    };
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    
    let system = Test1System {
      complete_table: vec![
        Test1Word {
          specific_paths: [
            (
              HLSubstitution { key: vec![A], sub_start: 0, sub_end: 1, sub_content: vec![Ae], at_start: false, at_end: false },
              vec![
              Test1Estimate { best_possible: 0.0, worst_possible: 1.0 },
              Test1Estimate { best_possible: 0.0, worst_possible: 0.0 }
              ],
            )
          ].iter().cloned().collect(),
          default_path: vec![
            Test1Estimate { best_possible: 5.0, worst_possible: 6.0 },
            Test1Estimate { best_possible: 7.0, worst_possible: 8.0 }
          ]
        },
        Test1Word {
          specific_paths: [].iter().cloned().collect(),
          default_path: vec![
            Test1Estimate { best_possible: 5.0, worst_possible: 6.0 },
            Test1Estimate { best_possible: 7.0, worst_possible: 8.0 }
          ]
        },
      ]
    };
    
    println!("Initializing ref data...");
    let r = init_ref_data(&system, &dictionary, &init_hl_slist);
    
    println!("Initializing working data...");
    let mut w = init_working_data(&r);
    
    println!("current_distances = {:?}", r.current_distances);
    println!("introducing_order = {:?}", r.introducing_order);
    println!("");
    
    let mut the_winner: Option<HLSubstitution> = None;
    
    for _ in 0 .. 10 {
      if let Some(outcome) = step(&system, &r, &mut w, true) {
        if let Outcome::FoundImprovement(winner, _) = outcome {
          the_winner = Some(winner);
          break;
        }
        else {
          panic!("Failed to find improvement");
        }
      }
      println!("");
    }
    
    assert_eq!(the_winner, Some(HLSubstitution { key: vec![A], sub_start: 0, sub_end: 1, sub_content: vec![Ae], at_start: false, at_end: false }));
  }
  
  #[test]
  fn with_new_test_1() {
    use crate::glyphs::Glyph::*;
    let spelling = vec![S, O, M, E, T, H, I, N, G];
    let pronunciation = vec![S, Uh, M, Th, Ih, Ng];
    
    let hl_slist = HLSubstitutionList::from_debug_str("[ay]$→ϵ, [sh]→ʃ, [ed]$→d, [oul]d$→ɜ, [ce]$→s, [i]$→i, [yo]→y, [tio]n$→ʃʌ, [at]$→æt, [re]$→r, [an]→ʌn, [to]$→tu, [y]$→ɩ, [c]→k, ^[i]→ɪ, [s]$→z, [er]$→ʳ, [of]→ʌv, [ing]$→ɪŋ, [e]$→ʌ, [th]→ϑ").unwrap();
    
    let sub = HLSubstitution::from_debug_str("[thi]→ϑɪs").unwrap();
    
    let orig_transformed = apply_copied(&spelling, &hl_slist);
    println!("orig_transformed = {:?}", orig_transformed);
    
    let orig_dist = distance(&orig_transformed, &pronunciation);
    assert_eq!(orig_dist, 3);
    
    assert!(new_distance_if_new(&spelling, &pronunciation, &hl_slist, &sub).is_some());
  }
}

