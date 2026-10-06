
// vim: shiftwidth=2

/* The basic way this works is as follows:
 *
 * When you introduce an edit, look at the words before and after it
 * in the introducing order.
 * 
 * Words before it are expected to worsen by some amount---basicallly the
 * expected devation restricted to above zero.
 * 
 * Words after it are expected to worsen or improve by some amount, with 
 * the general restriction that the improvement can never make the distance
 * negative, so it must be so restricted.
 * 
 * We assume these are all indepnedent.
 * 
 * The word itself, of course, has known improvement.
 * 
 * Now, as you probe this edit, you'll run it on a word; you'll then know
 * the exact improvement or worsening for that word. The exact value, of course,
 * is just a shift in the mean with no variance.
 *
 * But you have to "take out" the gaussian that used to be there. So, look
 * to whether this was a "before" word or an "after" word in the introducing
 * order (it will randomly jump around because the frequency order will be
 * different than the introducing order).
 * 
 * Based on whether it's a "before" word or an "after" word, you can calculate
 * what the gaussian was.
 * 
 * Then, once you know the gaussian, you can de-mix it from the working gaussian.
 * 
 */

#[derive(Debug, Clone)]
pub struct Estimator {
  pub actual_weighted_change: f64,
  pub before_ss_freq: f64,
  pub after_ss_freq: f64,
  pub after_nonzero_ss_freq: f64,
}

pub struct ExpectationTable {
  ss_frequency_in_introducing_order: Vec<f64>,
  remaining_ss_frequency_in_introducing_order: Vec<f64>,
  remaining_nonzero_ss_frequency_in_introducing_order: Vec<f64>,
}

pub fn build_expectation_table(
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
  ) -> ExpectationTable
{
  let n = frequencies_in_introducing_order.len();
  
  let mut ss_frequency_in_introducing_order: Vec<f64> = vec![0.0; n];
  let mut sum = 0.0;
  for i in 0 .. n {
    sum += frequencies_in_introducing_order[i] * frequencies_in_introducing_order[i];
    ss_frequency_in_introducing_order[i] = sum;
  }
  
  let mut remaining_ss_frequency_in_introducing_order: Vec<f64> = vec![0.0; n];
  let mut sum = 0.0;
  for i in (0 .. n).rev() {
    sum += frequencies_in_introducing_order[i] * frequencies_in_introducing_order[i];
    remaining_ss_frequency_in_introducing_order[i] = sum;
  }
  
  let mut remaining_nonzero_ss_frequency_in_introducing_order: Vec<f64> = vec![0.0; n];
  let mut sum = 0.0;
  for i in (0 .. n).rev() {
    if current_distances_in_introducing_order[i] > 0 {
      sum += frequencies_in_introducing_order[i] * frequencies_in_introducing_order[i];
    }
    remaining_nonzero_ss_frequency_in_introducing_order[i] = sum;
  }
  
  ExpectationTable {
    ss_frequency_in_introducing_order,
    remaining_ss_frequency_in_introducing_order,
    remaining_nonzero_ss_frequency_in_introducing_order,
  }
}

pub fn estimate_introduce_edit(
  frequencies_in_introducing_order: &Vec<f64>,
  et: &ExpectationTable,
  introducing_i: usize) -> Estimator
{
  /* When you introduce an edit, look at the words before and after it
   * in the introducing order.
   * 
   * Words before it are expected to worsen by some amount---basicallly the
   * expected devation restricted to above zero.
   * 
   * Words after it are expected to worsen or improve by some amount, with 
   * the general restriction that the improvement can never make the distance
   * negative, so it must be so restricted.
   * 
   * We assume these are all indepnedent.
   * 
   * The word itself, of course, has known improvement.
   */
  
  let i = introducing_i;
  let n = frequencies_in_introducing_order.len();
  
  Estimator {
    actual_weighted_change: -1.0 * frequencies_in_introducing_order[i],
    before_ss_freq: if i > 0 { et.ss_frequency_in_introducing_order[i-1] } else { 0.0 },
    after_ss_freq: if i < n-1 { et.remaining_ss_frequency_in_introducing_order[i+1] } else { 0.0 },
    after_nonzero_ss_freq: if i < n-1 { et.remaining_nonzero_ss_frequency_in_introducing_order[i] } else { 0.0 },
  }
}

pub fn introduce_edit(
  frequencies_in_introducing_order: &Vec<f64>,
  et: &ExpectationTable,
  introducing_i: usize,
  change_at_introducing_i: i32) -> Estimator
{
  /* When you introduce an edit, look at the words before and after it
   * in the introducing order.
   * 
   * Words before it are expected to worsen by some amount---basicallly the
   * expected devation restricted to above zero.
   * 
   * Words after it are expected to worsen or improve by some amount, with 
   * the general restriction that the improvement can never make the distance
   * negative, so it must be so restricted.
   * 
   * We assume these are all indepnedent.
   * 
   * The word itself, of course, has known improvement.
   */
  
  let i = introducing_i;
  let n = frequencies_in_introducing_order.len();
  
  Estimator {
    actual_weighted_change: (change_at_introducing_i as f64) * frequencies_in_introducing_order[i],
    before_ss_freq: if i > 0 { et.ss_frequency_in_introducing_order[i-1] } else { 0.0 },
    after_ss_freq: if i < n-1 { et.remaining_ss_frequency_in_introducing_order[i+1] } else { 0.0 },
    after_nonzero_ss_freq: if i < n-1 { et.remaining_nonzero_ss_frequency_in_introducing_order[i+1] } else { 0.0 },
  }
}

pub fn update_edit(
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
    introduced_i: usize,
    updated_i: usize,
    change_at_updated_i: i32,
    working_expectation: &Estimator
  ) -> Estimator
{
  /* As you probe this edit, you'll run it on a word; you'll then know
   * the exact improvement or worsening for that word. The exact value, of course,
   * is just a shift in the mean with no variance.
   */
  let i = updated_i;
    
  let shift = (change_at_updated_i as f64) * frequencies_in_introducing_order[updated_i];
  let nonzero = current_distances_in_introducing_order[i] > 0;
  let before = updated_i < introduced_i;
  let same = updated_i == introduced_i;
  
  let f = frequencies_in_introducing_order[updated_i];
  let ff = f * f;
  
  let e = working_expectation;
  
  if same {
    e.clone()
  }
  else {
    Estimator {
      actual_weighted_change: e.actual_weighted_change + shift,
      before_ss_freq: if before { e.before_ss_freq - ff } else { e.before_ss_freq },
      after_ss_freq: if !before { e.after_ss_freq - ff } else { e.after_ss_freq },
      after_nonzero_ss_freq: if !before && nonzero { e.after_nonzero_ss_freq - ff } else { e.after_nonzero_ss_freq },
    }
  }
}

pub fn calc_best_possible(e: &Estimator, scale: f64) -> f64 {
  e.actual_weighted_change - scale * e.after_nonzero_ss_freq.max(0.0).sqrt()
}

pub fn calc_worst_possible(e: &Estimator, scale: f64) -> f64 {
  e.actual_weighted_change + scale * (e.before_ss_freq + e.after_ss_freq).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
  use super::*;
  
  struct ExampleTable1 {
    num_words: usize,
    introducing_order_rev: Vec<usize>,
    frequencies_in_introducing_order: Vec<f64>,
    current_distances_in_introducing_order: Vec<u32>,
    table: ExpectationTable
  }
  fn example_table_1() -> ExampleTable1 {
    let num_words: usize = 5;
    
    let frequencies: Vec<f64> = vec![1.0, 0.5, 0.25, 0.125, 0.0625];
    let current_distances: Vec<u32> = vec![0, 2, 1, 3, 1];
    
    let introducing_order: Vec<usize> = vec![1, 3, 2, 4, 0];
    
    let mut introducing_order_rev: Vec<usize> = vec![0; num_words];
    for i in 0 .. num_words {
      let j = introducing_order[i];
      introducing_order_rev[j] = i;
    }
    let introducing_order_rev = introducing_order_rev;
    
    let frequencies_in_introducing_order = (0 .. num_words).map(|i| frequencies[introducing_order[i]]).collect();
    let current_distances_in_introducing_order = (0 .. num_words).map(|i| current_distances[introducing_order[i]]).collect();
    
    let table = build_expectation_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order);
    
    ExampleTable1 {
      num_words, introducing_order_rev, frequencies_in_introducing_order, current_distances_in_introducing_order, table
    }
  }
  
  #[test]
  fn build_expectation_table_test_1() {
    use more_asserts::assert_lt;

    let ExampleTable1 { table, .. } = example_table_1();
    
    println!("ss_frequency_in_introducing_order = {:?}", table.ss_frequency_in_introducing_order);
    
    // 0.25      , 0.265625  , 0.328125  , 0.33203125, 1.33203125
    assert_lt!((table.ss_frequency_in_introducing_order[0] - 0.25).abs(), 0.01);
    assert_lt!((table.ss_frequency_in_introducing_order[1] - 0.265625).abs(), 0.01);
    assert_lt!((table.ss_frequency_in_introducing_order[2] - 0.328125).abs(), 0.01);
    assert_lt!((table.ss_frequency_in_introducing_order[3] - 0.33203125).abs(), 0.01);
    assert_lt!((table.ss_frequency_in_introducing_order[4] - 1.33203125).abs(), 0.01);
    
    // 1.33203125, 1.08203125, 1.06640625, 1.00390625, 1.        
    assert_lt!((table.remaining_ss_frequency_in_introducing_order[0] - 1.33203125).abs(), 0.01);
    assert_lt!((table.remaining_ss_frequency_in_introducing_order[1] - 1.08203125).abs(), 0.01);
    assert_lt!((table.remaining_ss_frequency_in_introducing_order[2] - 1.06640625).abs(), 0.01);
    assert_lt!((table.remaining_ss_frequency_in_introducing_order[3] - 1.00390625).abs(), 0.01);
    assert_lt!((table.remaining_ss_frequency_in_introducing_order[4] - 1.).abs(), 0.01);
    
    // 0.33203125, 0.08203125, 0.06640625, 0.00390625, 0.        
    assert_lt!((table.remaining_nonzero_ss_frequency_in_introducing_order[0] - 0.33203125).abs(), 0.01);
    assert_lt!((table.remaining_nonzero_ss_frequency_in_introducing_order[1] - 0.08203125).abs(), 0.01);
    assert_lt!((table.remaining_nonzero_ss_frequency_in_introducing_order[2] - 0.06640625).abs(), 0.01);
    assert_lt!((table.remaining_nonzero_ss_frequency_in_introducing_order[3] - 0.00390625).abs(), 0.01);
    assert_lt!((table.remaining_nonzero_ss_frequency_in_introducing_order[4] - 0.0).abs(), 0.01);
  }
  
  #[test]
  fn goes_to_zero_test_1() {
    let ExampleTable1 { num_words, table, frequencies_in_introducing_order, current_distances_in_introducing_order, introducing_order_rev, .. } = example_table_1();
    
    let introduced_i = 0;
    
    let mut ex = introduce_edit(&frequencies_in_introducing_order, &table, introduced_i, 0);
    
    for j in 0 .. num_words {
      let i = introducing_order_rev[j];
      if i != introduced_i {
        ex = update_edit(&frequencies_in_introducing_order, &current_distances_in_introducing_order, introduced_i, i, 0, &ex);
      }
    }
    
    assert!(ex.after_ss_freq.abs() < 0.01);
    assert!(ex.before_ss_freq.abs() < 0.01);
    assert!(ex.after_nonzero_ss_freq.abs() < 0.01);
  }
  
  #[test]
  fn goes_to_zero_test_2() {
    let ExampleTable1 { num_words, table, frequencies_in_introducing_order, current_distances_in_introducing_order, introducing_order_rev, .. } = example_table_1();
    
    for introduced_i in 0 .. num_words {
      let mut ex = introduce_edit(&frequencies_in_introducing_order, &table, introduced_i, 0);
      
      for j in 0 .. num_words {
        let i = introducing_order_rev[j];
        if i != introduced_i {
          ex = update_edit(&frequencies_in_introducing_order, &current_distances_in_introducing_order, introduced_i, i, 0, &ex);
        }
      }
      
      assert!(ex.after_ss_freq.abs() < 0.01);
      assert!(ex.before_ss_freq.abs() < 0.01);
      assert!(ex.after_nonzero_ss_freq.abs() < 0.01);
    }
  }
}


