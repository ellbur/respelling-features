
use keyed_priority_queue::KeyedPriorityQueue;
use float_ord::FloatOrd;
use std::collections::HashMap;
use rayon::prelude::*;
use noisy_float::prelude::*;
use std::hash::Hash;
use core::fmt::Debug;

#[derive(Clone)]
pub struct SubWithImprovement<Edit: Eq + PartialEq> {
  pub sub: Edit,
  pub improvement: u32,
  pub size_cost: f64
}

// Words are identified by their index into the frequencies passed to
// init_ref_data. The EditSystem holds whatever it needs to know about each
// word (e.g. spellings and pronunciations already transformed by prior rules),
// so a new one is built for each search.
pub trait EditSystem<Edit: Eq + PartialEq> {
  fn find_improving_edits(&self, word: usize) -> Vec<SubWithImprovement<Edit>>;
  
  fn distance(&self, word: usize) -> u32;
  fn new_distance(&self, new_rule: &Edit, word: usize) -> Option<u32>;
  
  // For debugging output.
  fn describe_word(&self, word: usize) -> String {
    format!("#{}", word)
  }
}

pub trait Table<Estimate: Clone, Estimator: Clone, Edit> {
  fn introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize,
    change_at_introducing_i: i32,
    edit: &Edit
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

pub trait EstimationSystem<T: Table<Estimate, Estimator, Edit>, Estimate: Clone, Estimator: Clone, Edit> {
  fn build_table(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>
  ) -> T;
  
  fn calc_best_possible(&self, estimate: &Estimate) -> R64;
  fn calc_worst_possible(&self, estimate: &Estimate) -> R64;
  
  fn calc_estimate(&self, estimator: &Estimator) -> Estimate;
}

#[derive(Debug)]
pub enum Outcome<Edit: Debug> {
  FoundImprovement(Edit, f64),
  FailedToFindImprovement(Edit, f64),
  // No word had any candidate edit at all.
  NoCandidates
}

#[derive(Debug, Clone)]
pub struct WorkingEntry<Estimator> {
  estimator: Estimator,
  best_possible: f64,
  worst_possible: f64,
  size_cost: f64,
  
  // This is an index into the introducing vector,
  // which is ordered by weighted badness.
  introducing_index: usize,
  
  // This is an index into the exploring vector,
  // which is ordered by frequency.
  next_to_explore_index: usize
}

pub struct ReferenceData<'d, T> {
  pub(crate) frequencies: &'d [f64],
  
  pub(crate) n: usize,
  pub(crate) current_distances: Vec<u32>,
  
  pub(crate) table: T,
  
  pub(crate) introducing_order: Vec<usize>,
  pub(crate) introducing_order_rev: Vec<usize>,
  
  pub(crate) frequencies_in_introducing_order: Vec<f64>,
  pub(crate) current_distances_in_introducing_order: Vec<u32>,
  
  // Each round of work advances up to edit_chunk_size edits (in parallel)
  // by up to steps_chunk_size words each.
  pub edit_chunk_size: usize,
  pub steps_chunk_size: usize
}

pub fn init_ref_data<
    'd,
    'h,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit>,
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq,
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    frequencies: &'d [f64]
  ) -> ReferenceData<'d, T>
{
  let n = frequencies.len();

  let current_distances: Vec<u32> = (0 .. n).map(|j| {
    edit_sys.distance(j)
  }).collect();
  
  let mut introducing_order: Vec<usize> = (0 .. n).collect();
  introducing_order.sort_by_key(|&i| FloatOrd(-frequencies[i] * (current_distances[i] as f64)));
  let introducing_order = introducing_order;
  
  let mut introducing_order_rev = vec![0; n];
  for i in 0 .. n {
    let j = introducing_order[i];
    introducing_order_rev[j] = i;
  }

  let frequencies_in_introducing_order = (0 .. n).map(|i| frequencies[introducing_order[i]]).collect();
  
  let current_distances_in_introducing_order = (0 .. n).map(|i| current_distances[introducing_order[i]]).collect();
  
  let table = est_sys.build_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order);
  
  ReferenceData {
    frequencies,
    
    n,
    current_distances,
    table,
    
    introducing_order,
    introducing_order_rev,
    
    frequencies_in_introducing_order,
    current_distances_in_introducing_order,
    
    edit_chunk_size: 256,
    steps_chunk_size: 256
  }
}

pub struct WorkingData<
    Estimator,
    Edit: Eq + PartialEq + Hash
  >
{
  working_table: HashMap<Edit, WorkingEntry<Estimator>>,
  best_possible: KeyedPriorityQueue<Edit, FloatOrd<f64>>,
  best_possible_rev: KeyedPriorityQueue<Edit, FloatOrd<f64>>,
  
  introducing_working_index: usize
}

// The edit currently leading the search, with its best and worst possible
// scores so far and whether it has been checked against every word.
pub fn leader<Estimator, Edit: Eq + PartialEq + Hash>(w: &WorkingData<Estimator, Edit>, n: usize) -> Option<(&Edit, f64, f64, bool)> {
  let (edit, _) = w.best_possible.peek()?;
  let entry = w.working_table.get(edit)?;
  Some((edit, entry.best_possible, entry.worst_possible, entry.next_to_explore_index >= n))
}

fn improving_edits_at_i<
  'd,
  T,
  Edit: Eq + PartialEq,
  EditSys: EditSystem<Edit>
>(
  edit_sys: &EditSys,
  r: &ReferenceData<'d, T>,
  i: usize
) -> Vec<SubWithImprovement<Edit>>
{
  let j = r.introducing_order[i];
  edit_sys.find_improving_edits(j)
}

pub fn init_working_data<Estimator, Edit: Eq + PartialEq + Hash>() -> WorkingData<Estimator, Edit> {
  let working_table: HashMap<Edit, WorkingEntry<Estimator>> = HashMap::new();
  let best_possible: KeyedPriorityQueue<Edit, FloatOrd<f64>> = KeyedPriorityQueue::new();
    
  // This stores working items in *reverse* order by best_possible. Items that cannot be as good
  // as the worst_possible of the top of the best_possible queue should be deleted.
  let best_possible_rev: KeyedPriorityQueue<Edit, FloatOrd<f64>> = KeyedPriorityQueue::new();
  
  WorkingData {
    working_table,
    best_possible,
    best_possible_rev,
    
    introducing_working_index: 0
  }
}

fn estimate_from_introducing_iterator<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    Edit: Eq + PartialEq + Hash,
  >(
    r: &ReferenceData<'d, T>,
    w: &mut WorkingData<Estimator, Edit>,
    _debug: bool
  ) -> Option<Estimate>
{
  if w.introducing_working_index < r.n {
    let i = w.introducing_working_index;
    Some(r.table.estimate_introduce(&r.frequencies_in_introducing_order, i))
  }
  else {
    None
  }
}

fn introduce<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit>,
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq + Hash + Clone,
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T>,
    w: &mut WorkingData<Estimator, Edit>,
    debug: bool
  )
{
  // This occurs if either: (1) there is nothing in the heap, or (2) the
  // heap top's best_possible is not as good as best_possible_from_introducing_iterator.
  
  // Add it to the working table.
  // Note that we need to consider the introducing_index in initializing
  // best_possible and worst_possible.
  
  if w.introducing_working_index >= r.n {
    return;
  }
  
  let introducing_improving_edits = improving_edits_at_i(edit_sys, r, w.introducing_working_index);
   
  let i = w.introducing_working_index;
  
  for edit in introducing_improving_edits {
    if !w.working_table.contains_key(&edit.sub) {
      let improvement = edit.improvement;
      let change = -(improvement as i32);
      
      let init_estimator: Estimator = r.table.introduce(&r.frequencies_in_introducing_order, i, change, &edit.sub);
      let e = est_sys.calc_estimate(&init_estimator);
      let size_cost = edit.size_cost;
      let best_possible = est_sys.calc_best_possible(&e).raw() + size_cost;
      let worst_possible = est_sys.calc_worst_possible(&e).raw() + size_cost;
      
      let entry = WorkingEntry {
        estimator: init_estimator,
        best_possible,
        worst_possible,
        introducing_index: w.introducing_working_index,
        next_to_explore_index: 0,
        size_cost: edit.size_cost
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

fn cull_working_table<'d, 'h, Estimator, Edit: Eq + PartialEq + Hash + Debug>(w: &mut WorkingData<Estimator, Edit>, debug: bool) {
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

fn new_distance_if_new<
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq + Hash,
  >(
    edit_sys: &EditSys,
    edit: &Edit,
    word: usize
  ) -> Option<u32>
{
  edit_sys.new_distance(edit, word)
}

fn change_with_new<
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq + Hash,
  >(
    edit_sys: &EditSys,
    edit: &Edit,
    word: usize,
    orig_dist: u32
  ) -> i32
{
  match new_distance_if_new(edit_sys, edit, word) {
    Some(new_dist) => (new_dist as i32) - (orig_dist as i32),
    None => 0
  }
}

fn calc_change<
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq + Hash,
  >(
    edit_sys: &EditSys,
    edit: &Edit,
    word: usize,
    orig_dist: u32
  ) -> i32
{
  change_with_new(edit_sys, edit, word, orig_dist)
}

pub(crate) fn produce_outcome<Edit: Debug + Clone>(best_possible_sub: &Edit, best_possible: f64) -> Outcome<Edit> {
  if best_possible < 0.0 {
    Outcome::FoundImprovement(best_possible_sub.clone(), best_possible)
  }
  else {
    Outcome::FailedToFindImprovement(best_possible_sub.clone(), best_possible)
  }
}

fn check_for_winner<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    Edit: Debug + Hash + Eq + Clone,
  >(
    r: &ReferenceData<'d, T>,
    w: &WorkingData<Estimator, Edit>,
    best_possible_from_introducing_iterator: Option<f64>,
    debug: bool
  ) -> Option<Outcome<Edit>>
{
  let best_possible_sub = w.best_possible.peek().unwrap().0.clone();
  
  if debug { println!("advancing {:?}", best_possible_sub); }
  
  let working_table_len = w.working_table.len();
  let working = w.working_table.get(&best_possible_sub).unwrap();
  
  if working_table_len == 1 && best_possible_from_introducing_iterator.map_or_else(|| true, |b| working.worst_possible <= b) {
    if debug { println!("Found the winner by priority: {:?} {}/{} {} {} {:?}", best_possible_sub, working.next_to_explore_index, r.n, working.best_possible, working.worst_possible, best_possible_from_introducing_iterator); }
    Some(produce_outcome(&best_possible_sub, working.best_possible))
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
      Some(produce_outcome(&best_possible_sub, working.best_possible))
    }
    else {
      None
    }
  }
}

struct WorkIntermediate<E> {
  estimator: E,
  next_to_explore_index: usize
}

fn advance_many<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EditSys: EditSystem<Edit>,
    Edit: Eq + PartialEq + Hash
  >(
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T>,
    sub: &Edit,
    working: &WorkingEntry<Estimator>,
    num_to_advance: usize,
    debug: bool
  ) -> WorkIntermediate<Estimator>
{
  let mut j = working.next_to_explore_index;
  let mut k = 0;
  let mut estimator = working.estimator.clone();
  
  while k < num_to_advance && j < r.n {
    let i = r.introducing_order_rev[j];
    
    if i == working.introducing_index {
      // Nothing to do, we've already seen this one
    }
    else {
      let change = calc_change(
        edit_sys,
        &sub,
        j,
        r.current_distances[j]
      );
      
      if debug {
        println!("  old distance = {}", r.current_distances[j]);
        println!("  change = {}", change);
      }
      
      estimator = r.table.update_edit(
        &r.frequencies_in_introducing_order,
        &r.current_distances_in_introducing_order,
        working.introducing_index,
        i,
        change,
        estimator
      );
    }
    
    k += 1;
    j += 1;
  }
  
  WorkIntermediate {
    estimator,
    next_to_explore_index: j
  }
}

struct AdvancingWork<Estimator, Edit> {
  edit: Edit,
  working: WorkingEntry<Estimator>
}

struct AdvancingResult<Estimator, Edit> {
  edit: Edit,
  estimator: Estimator,
  best_possible: f64,
  worst_possible: f64,
  next_to_explore_index: usize
}

fn do_work<
    'd,
     T: Table<Estimate, Estimator, Edit> + Send + Sync,
     Estimate: Clone + Send + Sync,
     Estimator: Clone + Send + Sync,
     EstSys: EstimationSystem<T, Estimate, Estimator, Edit> + Send + Sync,
     EditSys: EditSystem<Edit> + Send + Sync,
     Edit: Eq + PartialEq + Hash + Send + Sync + Debug + Clone,
   >(
     est_sys: &EstSys,
     edit_sys: &EditSys,
     r: &ReferenceData<'d, T>,
     w: &mut WorkingData<Estimator, Edit>,
     debug: bool
   )
{
  // To take advantage of multiple CPU cores, we process edits in chunks
  let edit_chunk_size = r.edit_chunk_size;
  let steps_chunk_size = r.steps_chunk_size;
  
  // Go explore the next word and update best_possible and worst_possible accordingly. Note
  // that in updating best_possible, we need to consider this entry's introducing_index,
  // since that will tell us which words it could conceivably improve.
  
  let mut edit_chunk: Vec<AdvancingWork<Estimator, Edit>> = vec![];
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
  
  let result_chunk: Vec<AdvancingResult<Estimator, Edit>> = edit_chunk.into_par_iter().map(|work| {
    let best_possible_sub = work.edit;
    let working = work.working;
    
    if debug { println!("advancing {:?}", best_possible_sub); }
    
    let intermediate = advance_many(edit_sys, r, &best_possible_sub, &working, steps_chunk_size, debug);
    
    let e = est_sys.calc_estimate(&intermediate.estimator);
    let size_cost = working.size_cost;
    let best_possible = est_sys.calc_best_possible(&e).raw() + size_cost;
    let worst_possible = est_sys.calc_worst_possible(&e).raw() + size_cost;
    
    AdvancingResult {
      edit: best_possible_sub,
      estimator: intermediate.estimator,
      best_possible,
      worst_possible,
      next_to_explore_index: intermediate.next_to_explore_index
    }
  }).collect();
  
  for result in result_chunk {
    w.best_possible_rev.set_priority(&result.edit, FloatOrd(result.best_possible)).unwrap();
    w.best_possible.push(result.edit.clone(), FloatOrd(-result.best_possible));
    
    let working = w.working_table.get_mut(&result.edit).unwrap();
    working.estimator = result.estimator;
    working.best_possible = result.best_possible;
    working.worst_possible = result.worst_possible;
    working.next_to_explore_index = result.next_to_explore_index;
  }
}

fn dump_state<
    'd,
    T,
    Estimator,
    EditSys: EditSystem<Edit>,
    Edit: Debug + Eq + PartialEq + Hash
  >(
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T>,
    w: &mut WorkingData<Estimator, Edit>
  )
{
  for j in 0 .. (r.n+1) {
    for (edit, w) in w.working_table.iter() {
      if w.next_to_explore_index == j {
        println!("          {:>10} bp={:>4.1} wp={:>4.1}", format!("{:?}", edit), w.best_possible, w.worst_possible);
      }
    }
    if j < r.n {
      println!("  {:>4} f={:>4.2} d={:>3}", edit_sys.describe_word(j), r.frequencies[j], r.current_distances[j]);
    }
  }
}

pub fn step<
    'd,
    T: Table<Estimate, Estimator, Edit> + Send + Sync,
    Estimate: Clone + Send + Sync,
    Estimator: Clone + Send + Sync,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit> + Send + Sync,
    EditSys: EditSystem<Edit> + Send + Sync,
    Edit: Eq + PartialEq + Hash + Debug + Clone + Send + Sync,
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T>,
    w: &mut WorkingData<Estimator, Edit>,
    debug: bool
  ) -> Option<Outcome<Edit>>
{
  if debug { println!("step"); }
  
  if debug { dump_state(edit_sys, r, w); }
  
  if w.working_table.is_empty() {
    if w.introducing_working_index >= r.n {
      if debug { println!("Working table is empty and every word has been introduced."); }
      return Some(Outcome::NoCandidates);
    }
    if debug { println!("Working table is empty."); }
    introduce(est_sys, edit_sys, r, w, debug);
    None
  }
  else {
    // Cull the working table by starting with the top of best_possible_rev.
    // This means that once the working table is down to a single element, and that element's
    // worst_possible is better than best_possible_from_introducing_iterator, the algorithm
    // may terminate.
    cull_working_table(w, debug);
    
    let estimate_from_introducing_iterator = estimate_from_introducing_iterator(r, w, debug);
    let best_possible_from_introducing_iterator = estimate_from_introducing_iterator.map(|e| est_sys.calc_best_possible(&e).raw());
    
    let best_possible_top = w.best_possible.peek().unwrap();
    let best_possible_from_working_table = -best_possible_top.1.0;
    
    if debug { println!("best_possible_from_working_table = {}", best_possible_from_working_table); }
    
    let intro_iter_is_better = best_possible_from_introducing_iterator.map_or_else(|| false, |b| b < best_possible_from_working_table);
    
    if debug { println!("Best possible:"); }
    if debug { println!("  from old ones: {:.2} ({:?}) {}", best_possible_from_working_table, best_possible_top.0, if intro_iter_is_better {""} else {"*"}); }
    
    if intro_iter_is_better {
      introduce(est_sys, edit_sys, r, w, debug);
      None
    }
    else {
      if let Some(res) = check_for_winner(r, w, best_possible_from_introducing_iterator, debug) {
        Some(res)
      }
      else {
        do_work(est_sys, edit_sys, r, w, debug);
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
  
  #[derive(PartialEq, Eq, Hash, Clone, Debug)]
  struct Test1Edit {
    name: String
  }
  
  
  #[derive(Clone)]
  struct Test1Word {
    specific_paths: HashMap<Test1Edit, Vec<Test1Estimate>>,
    default_path: Vec<Test1Estimate>
  }
  
  struct Test1Table {
    complete_table: Vec<Test1Word>
  }
  
  impl Table<Test1Estimate, Test1Estimator, Test1Edit> for Test1Table {
    fn introduce(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      introducing_i: usize,
      _change_at_introducing_i: i32,
      edit: &Test1Edit
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

  struct Test1EstimationSystem {
    complete_table: Vec<Test1Word>
  }
  
  impl EstimationSystem<Test1Table, Test1Estimate, Test1Estimator, Test1Edit> for Test1EstimationSystem {
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
  
  struct Test1EditSystem {
    words: Vec<String>,
    improving_edits: HashMap<usize, Vec<SubWithImprovement<Test1Edit>>>,
    old_distances: HashMap<usize, u32>,
    new_distances: HashMap<(usize, Test1Edit), Option<u32>>,
  }
  
  impl EditSystem<Test1Edit> for Test1EditSystem {
    fn find_improving_edits(&self, word: usize) -> Vec<SubWithImprovement<Test1Edit>> {
      self.improving_edits.get(&word).unwrap().clone()
    }
    
    fn distance(&self, word: usize) -> u32 {
      self.old_distances.get(&word).unwrap().clone()
    }
    
    fn new_distance(&self, new_rule: &Test1Edit, word: usize) -> Option<u32> {
      self.new_distances.get(&(word, new_rule.clone())).unwrap().clone()
    }
    
    fn describe_word(&self, word: usize) -> String {
      self.words[word].clone()
    }
  }
  
  #[test]
  fn genastarlike_test_1() {
    let edit_sys = Test1EditSystem {
      words: vec!["to".to_owned(), "who".to_owned()],
      improving_edits: vec![
        (
          0,
          vec![
            SubWithImprovement {
              sub: Test1Edit { name: "foo".to_owned() },
              improvement: 1,
              size_cost: 0.0
            },
            SubWithImprovement {
              sub: Test1Edit { name: "bar".to_owned() },
              improvement: 1,
              size_cost: 0.0
            },
          ]
        )
      ].into_iter().collect(),
      
      old_distances: vec![
        (0, 1),
        (1, 1),
      ].into_iter().collect(),
      
      new_distances: vec![
        (
          ( 0, Test1Edit { name: "foo".to_owned() }),
          Some(0)
        ),
        (
          ( 0, Test1Edit { name: "bar".to_owned() }),
          Some(0)
        ),
        (
          ( 1, Test1Edit { name: "foo".to_owned() }),
          Some(0)
        ),
        (
          ( 1, Test1Edit { name: "bar".to_owned() }),
          Some(1)
        ),
      ].into_iter().collect()
    };
    
    let est_sys = Test1EstimationSystem {
      complete_table: vec![
        Test1Word {
          specific_paths: [
            (
              Test1Edit { name: "foo".to_owned() },
              vec![
                Test1Estimate { best_possible: -1.0, worst_possible:  1.0 },
                Test1Estimate { best_possible: -1.0, worst_possible: -1.0 }
              ],
            ),
            (
              Test1Edit { name: "bar".to_owned() },
              vec![
                Test1Estimate { best_possible: -1.0, worst_possible:  1.0 },
                Test1Estimate { best_possible: -0.5, worst_possible: -0.5 }
              ],
            ),
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
    
    // Word 0 is "to" and word 1 is "who".
    let frequencies = vec![1.0, 1.0];
    
    println!("Initializing ref data...");
    
    let r = init_ref_data(&est_sys, &edit_sys, &frequencies);
    
    println!("Initializing working data...");
    let mut w = init_working_data();
    
    println!("current_distances = {:?}", r.current_distances);
    println!("introducing_order = {:?}", r.introducing_order);
    println!("");
    
    let mut the_winner: Option<Test1Edit> = None;
    
    for _ in 0 .. 10 {
      if let Some(outcome) = step(&est_sys, &edit_sys, &r, &mut w, true) {
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
    
    assert_eq!(the_winner.unwrap(), Test1Edit { name: "foo".to_owned() });
  }
}

