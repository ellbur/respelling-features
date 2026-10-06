// genastarlike's search, spread over several threads with little
// coordination between them.
//
// Candidate edits are partitioned among the threads by hash: each thread owns
// its partition's table and heap outright. The search runs in rounds. In each
// round, every thread in parallel:
//   1. takes in the new candidates other threads sent it last round (in a
//      fixed order, so the result doesn't depend on timing),
//   2. drops candidates whose best possible score can't beat the current
//      leader's worst possible score (the shared bound),
//   3. introduces the words assigned to it, sending each candidate to the
//      thread that owns it,
//   4. advances its own best candidates by a batch of words each,
//   5. reports its best candidate and how many it has.
// Between rounds, one thread combines the reports, applies genastarlike's
// test for a winner, sets the shared bound, and decides which words to
// introduce next (as genastarlike does, while the next word could still
// produce a candidate better than the leader).
//
// The result depends only on the number of threads, not on timing. It can
// differ from genastarlike's in a close race, because candidates are
// explored in a different order.

use core::fmt::Debug;
use float_ord::FloatOrd;
use keyed_priority_queue::KeyedPriorityQueue;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Barrier, Mutex};

use crate::genastarlike::{produce_outcome, EditSystem, EstimationSystem, Outcome, ReferenceData, Table};

#[derive(Clone, Copy, Debug)]
pub struct ParParams {
  pub threads: usize,
  // Each round, each thread advances up to this many of its best
  // candidates...
  pub edits_per_round: usize,
  // ... by up to this many words each.
  pub steps_per_round: usize,
  // At most this many words are introduced per round (spread over the
  // threads).
  pub max_introduce_per_round: usize
}

impl ParParams {
  pub fn with_threads(threads: usize) -> ParParams {
    ParParams { threads, edits_per_round: 64, steps_per_round: 256, max_introduce_per_round: 4 * threads }
  }
}

#[derive(Clone)]
struct Entry<Estimator> {
  estimator: Estimator,
  best_possible: f64,
  worst_possible: f64,
  size_cost: f64,
  introducing_index: usize,
  next_to_explore_index: usize
}

struct Incoming<Edit> {
  edit: Edit,
  improvement: u32,
  size_cost: f64,
  introducing_index: usize,
  // Order among the candidates from the same word.
  seq: usize
}

struct Partition<Estimator, Edit: Hash + Eq> {
  table: HashMap<Edit, Entry<Estimator>>,
  heap: KeyedPriorityQueue<Edit, FloatOrd<f64>>
}

#[derive(Clone)]
struct Summary<Edit> {
  // The partition's best candidate: best and worst possible scores, and
  // whether it has been checked against every word.
  top: Option<(Edit, f64, f64, bool)>,
  count: usize
}

// What the coordinator tells every thread for the next round.
#[derive(Clone)]
struct Plan<Edit> {
  // Candidates whose best possible score is at least this are dropped...
  cull_above: f64,
  // ... except this one, the overall leader.
  leader: Option<Edit>,
  // Words (as introducing-order indices) to introduce, per thread.
  introduce: Vec<Vec<usize>>,
  done: bool
}

fn owner<Edit: Hash>(edit: &Edit, threads: usize) -> usize {
  let mut h = DefaultHasher::new();
  edit.hash(&mut h);
  (h.finish() % threads as u64) as usize
}

pub fn search<
    'd,
    T: Table<Estimate, Estimator, Edit> + Send + Sync,
    Estimate: Clone + Send + Sync,
    Estimator: Clone + Send + Sync,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit> + Send + Sync,
    EditSys: EditSystem<Edit> + Send + Sync,
    Edit: Eq + PartialEq + Hash + Debug + Clone + Send + Sync
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T>,
    params: ParParams
  ) -> Outcome<Edit>
{
  let p = params.threads;
  let n = r.n;

  let inboxes: Vec<Mutex<Vec<Incoming<Edit>>>> = (0 .. p).map(|_| Mutex::new(vec![])).collect();
  let summaries: Vec<Mutex<Summary<Edit>>> = (0 .. p).map(|_| Mutex::new(Summary { top: None, count: 0 })).collect();
  let plan: Mutex<Plan<Edit>> = Mutex::new(Plan { cull_above: f64::INFINITY, leader: None, introduce: vec![vec![]; p], done: false });
  let outcome: Mutex<Option<Outcome<Edit>>> = Mutex::new(None);
  // Introducing-order index of the next word to introduce.
  let next_intro: Mutex<usize> = Mutex::new(0);
  let barrier = Barrier::new(p);

  let intro_best = |i: usize| -> Option<f64> {
    if i < n {
      Some(est_sys.calc_best_possible(&r.table.estimate_introduce(&r.frequencies_in_introducing_order, i)).raw())
    }
    else {
      None
    }
  };

  // Runs between rounds, on one thread: checks for a winner and plans the
  // next round.
  let coordinate = || {
    let summaries: Vec<Summary<Edit>> = summaries.iter().map(|s| s.lock().unwrap().clone()).collect();
    let total: usize = summaries.iter().map(|s| s.count).sum();
    let leader = summaries.iter().filter_map(|s| s.top.clone())
      .min_by(|a, b| FloatOrd(a.1).cmp(&FloatOrd(b.1)));
    let mut next = next_intro.lock().unwrap();
    let next_best = intro_best(*next);
    // Candidates introduced last round only reach their owners' tables next
    // round, so nothing is decided while any are still in transit.
    let in_transit = inboxes.iter().any(|b| !b.lock().unwrap().is_empty());

    let mut introduce_count = 0;
    match &leader {
      None => {
        if in_transit {
          // Nothing to decide yet; the candidates arrive next round.
        }
        else if *next >= n {
          *outcome.lock().unwrap() = Some(Outcome::NoCandidates);
          plan.lock().unwrap().done = true;
          return;
        }
        else {
          // Nothing to work on: introduce words until something turns up.
          introduce_count = params.max_introduce_per_round;
        }
      },
      Some((edit, best, worst, explored)) => {
        let intro_is_better = next_best.map_or(false, |b| b < *best);
        if !intro_is_better && !in_transit {
          // genastarlike's test for a winner.
          if (total == 1 && next_best.map_or(true, |b| *worst <= b)) || *explored {
            *outcome.lock().unwrap() = Some(produce_outcome(edit, *best));
            plan.lock().unwrap().done = true;
            return;
          }
        }
        // Introduce while the next word could still beat the leader.
        while introduce_count < params.max_introduce_per_round
          && intro_best(*next + introduce_count).map_or(false, |b| b < *best)
        {
          introduce_count += 1;
        }
      }
    }

    let mut introduce: Vec<Vec<usize>> = vec![vec![]; p];
    for k in 0 .. introduce_count {
      if *next < n {
        introduce[k % p].push(*next);
        *next += 1;
      }
    }

    let mut plan = plan.lock().unwrap();
    plan.cull_above = leader.as_ref().map_or(f64::INFINITY, |l| l.2);
    plan.leader = leader.map(|l| l.0);
    plan.introduce = introduce;
  };

  // The first round starts with no candidates.
  coordinate();

  std::thread::scope(|scope| {
    for me in 0 .. p {
      let (inboxes, summaries, plan, barrier, coordinate) = (&inboxes, &summaries, &plan, &barrier, &coordinate);
      scope.spawn(move || {
        // Run the edit system's own parallel code inline on this thread.
        let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        pool.install(|| {
          let mut part: Partition<Estimator, Edit> = Partition { table: HashMap::new(), heap: KeyedPriorityQueue::new() };

          loop {
            let this_round = plan.lock().unwrap().clone();
            if this_round.done {
              break;
            }

            // 1. Take in new candidates, in a fixed order.
            let mut incoming = std::mem::take(&mut *inboxes[me].lock().unwrap());
            incoming.sort_by_key(|c| (c.introducing_index, c.seq));
            for c in incoming {
              if part.table.contains_key(&c.edit) {
                continue;
              }
              let init = r.table.introduce(&r.frequencies_in_introducing_order, c.introducing_index, -(c.improvement as i32), &c.edit);
              let e = est_sys.calc_estimate(&init);
              let best_possible = est_sys.calc_best_possible(&e).raw() + c.size_cost;
              let worst_possible = est_sys.calc_worst_possible(&e).raw() + c.size_cost;
              part.heap.push(c.edit.clone(), FloatOrd(-best_possible));
              part.table.insert(c.edit, Entry {
                estimator: init,
                best_possible,
                worst_possible,
                size_cost: c.size_cost,
                introducing_index: c.introducing_index,
                next_to_explore_index: 0
              });
            }

            // 2. Drop candidates that can't beat the leader.
            let cull: Vec<Edit> = part.table.iter()
              .filter(|(edit, entry)| entry.best_possible >= this_round.cull_above && this_round.leader.as_ref() != Some(*edit))
              .map(|(edit, _)| edit.clone())
              .collect();
            for edit in cull {
              part.table.remove(&edit);
              part.heap.remove(&edit);
            }

            // 3. Introduce this thread's words, sending each candidate to its
            // owner.
            for &i in &this_round.introduce[me] {
              let word = r.introducing_order[i];
              let mut outgoing: Vec<Vec<Incoming<Edit>>> = (0 .. p).map(|_| vec![]).collect();
              for (seq, e) in edit_sys.find_improving_edits(word).into_iter().enumerate() {
                let o = owner(&e.sub, p);
                outgoing[o].push(Incoming { edit: e.sub, improvement: e.improvement, size_cost: e.size_cost, introducing_index: i, seq });
              }
              for (o, out) in outgoing.into_iter().enumerate() {
                if !out.is_empty() {
                  inboxes[o].lock().unwrap().extend(out);
                }
              }
            }

            // 4. Advance this partition's best candidates.
            let mut advanced = vec![];
            for _ in 0 .. params.edits_per_round {
              let Some((edit, _)) = part.heap.pop() else { break };
              let entry = part.table.get_mut(&edit).unwrap();
              if entry.next_to_explore_index >= n {
                // Fully explored; it goes back as it is.
                advanced.push(edit);
                continue;
              }
              let end = (entry.next_to_explore_index + params.steps_per_round).min(n);
              for j in entry.next_to_explore_index .. end {
                let i = r.introducing_order_rev[j];
                if i == entry.introducing_index {
                  continue;
                }
                let change = match edit_sys.new_distance(&edit, j) {
                  Some(d) => d as i32 - r.current_distances[j] as i32,
                  None => 0
                };
                entry.estimator = r.table.update_edit(
                  &r.frequencies_in_introducing_order,
                  &r.current_distances_in_introducing_order,
                  entry.introducing_index,
                  i,
                  change,
                  entry.estimator.clone()
                );
              }
              entry.next_to_explore_index = end;
              let e = est_sys.calc_estimate(&entry.estimator);
              entry.best_possible = est_sys.calc_best_possible(&e).raw() + entry.size_cost;
              entry.worst_possible = est_sys.calc_worst_possible(&e).raw() + entry.size_cost;
              advanced.push(edit);
            }
            for edit in advanced {
              let best = part.table[&edit].best_possible;
              part.heap.push(edit, FloatOrd(-best));
            }

            // 5. Report.
            let top = part.heap.peek().map(|(edit, _)| {
              let entry = &part.table[edit];
              (edit.clone(), entry.best_possible, entry.worst_possible, entry.next_to_explore_index >= n)
            });
            *summaries[me].lock().unwrap() = Summary { top, count: part.table.len() };

            if barrier.wait().is_leader() {
              coordinate();
            }
            barrier.wait();
          }
        });
      });
    }
  });

  outcome.into_inner().unwrap().unwrap()
}
