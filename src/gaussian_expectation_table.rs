
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

use crate::gaussians::Gaussian;
use more_asserts::assert_lt;
use noisy_float::prelude::*;

pub struct ExpectationTable {
  total_frequency_in_introducing_order: Vec<f64>,
  root_total_squared_frequency_in_introducing_order: Vec<f64>,
  unknown_edit_gaussians_in_introducing_order: Vec<Gaussian>,
  total_remaining_unknown_edit_gaussians_in_introducing_order: Vec<Gaussian>,
  expected_deviation_above_zero: Gaussian
}

pub fn build_expectation_table(
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
    typical_sigma: R64
  ) -> ExpectationTable
{
  let unknown_edit_gaussian = Gaussian {
    mean: r64(0.0),
    sigma: typical_sigma
  };
  
  let max_trunc_f64: f64 = (typical_sigma * r64(20.0) + r64(20.0)).ceil().into();
  let max_trunc = max_trunc_f64 as u32;
  let truncated_gaussian_cache: Vec<Gaussian> = (0 .. max_trunc).map(|k| {
    unknown_edit_gaussian.restrict_above(r64(-(k as f64)))
  }).collect();
  
  let n = frequencies_in_introducing_order.len();
  
  let mut total_frequency_in_introducing_order: Vec<f64> = vec![0.0; n];
  for i in 0 .. n {
    let prev = if i == 0 { 0.0 } else { total_frequency_in_introducing_order[i-1] };
    total_frequency_in_introducing_order[i] = prev + frequencies_in_introducing_order[i];
  }
  let total_frequency_in_introducing_order = total_frequency_in_introducing_order;
  
  let mut total_squared_frequency_in_introducing_order: Vec<f64> = vec![0.0; n];
  for i in 0 .. n {
    let prev = if i == 0 { 0.0 } else { total_squared_frequency_in_introducing_order[i-1] };
    let f = frequencies_in_introducing_order[i];
    total_squared_frequency_in_introducing_order[i] = prev + f*f;
  }
  let root_total_squared_frequency_in_introducing_order = total_squared_frequency_in_introducing_order.into_iter().map(|z| z.sqrt()).collect();
  
  let zg = Gaussian { mean: r64(0.0), sigma: r64(0.0) };
  let mut unknown_edit_gaussians_in_introducing_order: Vec<Gaussian> = vec![zg; n];
  for i in 0 .. n {
    let current_distance = current_distances_in_introducing_order[i];
    assert_lt!(current_distance as usize, truncated_gaussian_cache.len());
    let f = frequencies_in_introducing_order[i];
    unknown_edit_gaussians_in_introducing_order[i] = truncated_gaussian_cache[current_distance as usize].scale(f);
  }
  let unknown_edit_gaussians_in_introducing_order = unknown_edit_gaussians_in_introducing_order;
  
  let mut total_remaining_unknown_edit_gaussians_in_introducing_order: Vec<Gaussian> = vec![zg; n];
  for i in (0 .. n).rev() {
    let prev = if i == n-1 { zg } else { total_remaining_unknown_edit_gaussians_in_introducing_order[i + 1] };
    total_remaining_unknown_edit_gaussians_in_introducing_order[i] = prev.add_indep(&unknown_edit_gaussians_in_introducing_order[i]);
  }
  let total_remaining_unknown_edit_gaussians_in_introducing_order = total_remaining_unknown_edit_gaussians_in_introducing_order;
  
  let expected_deviation_above_zero = truncated_gaussian_cache[0];
  
  ExpectationTable {
    total_frequency_in_introducing_order,
    root_total_squared_frequency_in_introducing_order,
    unknown_edit_gaussians_in_introducing_order,
    total_remaining_unknown_edit_gaussians_in_introducing_order,
    expected_deviation_above_zero
  }
}

pub fn estimate_introduce_edit(
  frequencies_in_introducing_order: &Vec<f64>,
  et: &ExpectationTable,
  introducing_i: usize) -> Gaussian
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
  
  // Words before it are expected to worsen by some amount---basicallly the
  // expected devation restricted to above zero.    
  let before_gaussian = {
    if introducing_i > 0 {
      let z = et.expected_deviation_above_zero;
      Gaussian {
        mean: z.mean * et.total_frequency_in_introducing_order[introducing_i - 1],
        sigma: z.sigma * et.root_total_squared_frequency_in_introducing_order[introducing_i - 1]
      }
    }
    else {
      Gaussian {
        mean: r64(0.0),
        sigma: r64(0.0)
      }
    }
  };
   
  // Words after it are expected to worsen or improve by some amount, with 
  // the general restriction that the improvement can never make the distance
  // negative, so it must be so restricted.
  // 
  // Note that this includes the word itself, since we haven't calculated this
  // one's change yet.
  let after_gaussian = {
    if introducing_i < frequencies_in_introducing_order.len() {
      et.total_remaining_unknown_edit_gaussians_in_introducing_order[introducing_i]
    }
    else {
      Gaussian {
        mean: r64(0.0),
        sigma: r64(0.0)
      }
    }
  }; 
  
  before_gaussian.add_indep(&after_gaussian)
}

pub fn introduce_edit(
  frequencies_in_introducing_order: &Vec<f64>,
  et: &ExpectationTable,
  introducing_i: usize,
  change_at_introducing_i: i32) -> Gaussian
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
  
  // Words before it are expected to worsen by some amount---basicallly the
  // expected devation restricted to above zero.    
  let before_gaussian = {
    if introducing_i > 0 {
      let z = et.expected_deviation_above_zero;
      Gaussian {
        mean: z.mean * et.total_frequency_in_introducing_order[introducing_i - 1],
        sigma: z.sigma * et.root_total_squared_frequency_in_introducing_order[introducing_i - 1]
      }
    }
    else {
      Gaussian {
        mean: r64(0.0),
        sigma: r64(0.0)
      }
    }
  };
   
  // Words after it are expected to worsen or improve by some amount, with 
  // the general restriction that the improvement can never make the distance
  // negative, so it must be so restricted.
  let after_gaussian = {
    if introducing_i < frequencies_in_introducing_order.len() - 1 {
      et.total_remaining_unknown_edit_gaussians_in_introducing_order[introducing_i + 1]
    }
    else {
      Gaussian {
        mean: r64(0.0),
        sigma: r64(0.0)
      }
    }
  }; 
  
  // The word itself, of course, has known improvement.
  let expectation_at_word_itself = (change_at_introducing_i as f64) * frequencies_in_introducing_order[introducing_i];
  
  before_gaussian.add_indep(&after_gaussian).shift(expectation_at_word_itself)
}

pub fn update_edit(
    frequencies_in_introducing_order: &Vec<f64>,
    et: &ExpectationTable,
    introduced_i: usize,
    updated_i: usize,
    change_at_updated_i: i32,
    working_expectation: &Gaussian
  ) -> Gaussian
{
  /* As you probe this edit, you'll run it on a word; you'll then know
   * the exact improvement or worsening for that word. The exact value, of course,
   * is just a shift in the mean with no variance.
   */
  let shift = (change_at_updated_i as f64) * frequencies_in_introducing_order[updated_i];
  
  /* But you have to "take out" the gaussian that used to be there. So, look
   * to whether this was a "before" word or an "after" word in the introducing
   * order (it will randomly jump around because the frequency order will be
   * different than the introducing order).
   * 
   * Based on whether it's a "before" word or an "after" word, you can calculate
   * what the gaussian was.
   */
  let gaussian_to_demix: Gaussian = {
    if updated_i < introduced_i {
      et.expected_deviation_above_zero.scale(frequencies_in_introducing_order[updated_i])
    }
    else if updated_i > introduced_i {
      et.unknown_edit_gaussians_in_introducing_order[updated_i]
    }
    else {
      panic!("Not allowed to update at the introduced_i")
    }
  };
  
  /* Then, once you know the gaussian, you can de-mix it from the working gaussian.
   */
  working_expectation.remove_indep(&gaussian_to_demix).shift(shift)
}

#[cfg(test)]
mod tests {
  use super::*;
  
  struct ExampleTable1 {
    num_words: usize,
    introducing_order_rev: Vec<usize>,
    frequencies_in_introducing_order: Vec<f64>,
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
    
    let typical_sigma: R64 = r64(1.0);
      
    let table = build_expectation_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order, typical_sigma);
    
    ExampleTable1 {
      num_words, introducing_order_rev, frequencies_in_introducing_order, table
    }
  }
  
  #[test]
  fn build_expectation_table_test_1() {
    let ExampleTable1 { table, .. } = example_table_1();
    
    assert!(table.total_frequency_in_introducing_order.len() == 5);
    assert!((table.total_frequency_in_introducing_order[0] - 0.5000) < 0.01);
    assert!((table.total_frequency_in_introducing_order[1] - 0.6250) < 0.01);
    assert!((table.total_frequency_in_introducing_order[2] - 0.8750) < 0.01);
    assert!((table.total_frequency_in_introducing_order[3] - 0.9375) < 0.01);
    assert!((table.total_frequency_in_introducing_order[4] - 1.9375) < 0.01);
    
    assert!(table.unknown_edit_gaussians_in_introducing_order.len() == 5);
    assert!((table.unknown_edit_gaussians_in_introducing_order[0].mean - 0.027) < 0.01);
    assert!((table.unknown_edit_gaussians_in_introducing_order[1].mean - 0.000) < 0.01);
    assert!((table.unknown_edit_gaussians_in_introducing_order[2].mean - 0.071) < 0.01);
    assert!((table.unknown_edit_gaussians_in_introducing_order[3].mean - 0.017) < 0.01);
    assert!((table.unknown_edit_gaussians_in_introducing_order[4].mean - 0.797) < 0.01);
  }
  
  #[test]
  fn introduce_edit_test_1() {
    let ExampleTable1 { table, frequencies_in_introducing_order, .. } = example_table_1();
    
    let i1 = introduce_edit(&frequencies_in_introducing_order, &table, 0, -1);
    let i2 = introduce_edit(&frequencies_in_introducing_order, &table, 0, -2);

    println!("i1 = {:?}", i1);
    println!("i2 = {:?}", i2);
    
    assert!(((i1.mean - frequencies_in_introducing_order[0]) - i2.mean).abs() < 0.01);
    assert!((i1.sigma - i2.sigma).abs() < 0.01);
  }
  
  #[test]
  fn goes_to_zero_test_1() {
    let ExampleTable1 { num_words, table, frequencies_in_introducing_order, introducing_order_rev, .. } = example_table_1();
    
    let introduced_i = 0;
    
    let mut ex = introduce_edit(&frequencies_in_introducing_order, &table, introduced_i, 0);
    println!("ex = {:0.2} +/- {:0.2}", ex.mean, ex.sigma);
    println!("");
    
    for j in 0 .. num_words {
      let i = introducing_order_rev[j];
      if i != introduced_i {
        ex = update_edit(&frequencies_in_introducing_order, &table, introduced_i, i, 0, &ex);
        println!("[{}/{}] ex = {:0.2} +/- {:0.2}", j, i, ex.mean, ex.sigma);
      }
    }
    
    println!("");
    println!("ex = {:0.2} +/- {:0.2}", ex.mean, ex.sigma);
    
    assert!(ex.mean.abs() < 0.01);
    assert!(ex.sigma.abs() < 0.01);
  }
  
  #[test]
  fn goes_to_zero_test_2() {
    let ExampleTable1 { num_words, table, frequencies_in_introducing_order, introducing_order_rev, .. } = example_table_1();
    
    for introduced_i in 0 .. num_words {
      let mut ex = introduce_edit(&frequencies_in_introducing_order, &table, introduced_i, 0);
      println!("ex = {:0.2} +/- {:0.2}", ex.mean, ex.sigma);
      println!("");
      
      for j in 0 .. num_words {
        let i = introducing_order_rev[j];
        if i != introduced_i {
          ex = update_edit(&frequencies_in_introducing_order, &table, introduced_i, i, 0, &ex);
          println!("[{}/{}] ex = {:0.2} +/- {:0.2}", j, i, ex.mean, ex.sigma);
        }
      }
      
      println!("");
      println!("ex = {:0.2} +/- {:0.2}", ex.mean, ex.sigma);
      println!("");
      println!("");
      
      assert!(ex.mean.abs() < 0.01);
      assert!(ex.sigma.abs() < 0.01);
    }
  }
}

