
use crate::astarlike::{Table, System};
use crate::gaussian_expectation_table2::*;
use crate::gaussian_expectation_table2 as get2;
use crate::high_level_substitutions::HLSubstitution;
use noisy_float::prelude::*;

pub struct GaussianTable {
  table: ExpectationTable
}

impl Table<Estimator, Estimator> for GaussianTable {
  fn introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize,
    change_at_introducing_i: i32,
    _edit: &HLSubstitution
  ) -> Estimator {
    introduce_edit(frequencies_in_introducing_order, &self.table, introducing_i, change_at_introducing_i)
  }
  
  fn estimate_introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize
  ) -> Estimator {
    estimate_introduce_edit(frequencies_in_introducing_order, &self.table, introducing_i)
  }
  
  fn update_edit(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
    introduced_i: usize,
    updated_i: usize,
    change_at_updated_i: i32,
    prev_estimate: Estimator
  ) -> Estimator {
    update_edit(frequencies_in_introducing_order, current_distances_in_introducing_order, introduced_i, updated_i, change_at_updated_i, &prev_estimate)
  }
}

pub struct GaussianSystem {
  pub scale: f64
}

impl System<GaussianTable, Estimator, Estimator> for GaussianSystem {
  fn build_table(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>
  ) -> GaussianTable {
    GaussianTable {
      table: build_expectation_table(
        frequencies_in_introducing_order,
        current_distances_in_introducing_order
      )
    }
  }
  
  fn calc_estimate(&self, estimator: &Estimator) -> Estimator {
    estimator.clone()
  }
  
  fn calc_best_possible(&self, g: &Estimator) -> R64 {
    r64(get2::calc_best_possible(g, self.scale))
  }

  fn calc_worst_possible(&self, g: &Estimator) -> R64 {
    r64(get2::calc_worst_possible(g, self.scale))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  
  struct ExampleTable1 {
    num_words: usize,
    introducing_order_rev: Vec<usize>,
    frequencies_in_introducing_order: Vec<f64>,
    current_distances_in_introducing_order: Vec<u32>
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
    
    ExampleTable1 {
      num_words, introducing_order_rev, frequencies_in_introducing_order, current_distances_in_introducing_order
    }
  }
  
  #[test]
  fn goes_to_zero_test_3() {
    use crate::astarlike::{System, Table};
    
    let ExampleTable1 { num_words, frequencies_in_introducing_order, introducing_order_rev, current_distances_in_introducing_order, .. } = example_table_1();
    
    let system = GaussianSystem {
      scale: 1.0
    };
    
    let table = system.build_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order);
    
    for introduced_i in 0 .. num_words {
      let mut ex = table.introduce(&frequencies_in_introducing_order, introduced_i, 0, &HLSubstitution {
        key: vec![],
        at_end: false,
        at_start: false,
        sub_start: 0,
        sub_end: 0,
        sub_content: vec![]
      });
      
      for j in 0 .. num_words {
        let i = introducing_order_rev[j];
        if i != introduced_i {
          ex = table.update_edit(&frequencies_in_introducing_order, &current_distances_in_introducing_order, introduced_i, i, 0, ex);
        }
      }
      
      assert!(ex.after_ss_freq.abs() < 0.01);
      assert!(ex.before_ss_freq.abs() < 0.01);
      assert!(ex.after_nonzero_ss_freq.abs() < 0.01);
    }
  }
  
  #[test]
  fn gaussian_astarlike_test_1() {
    use crate::glyphs::Glyph::*;
    use crate::astarlike::*;
    use crate::high_level_substitutions::HLSubstitutionList;
    
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(5);
    let dictionary = dictionary;
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    
    let system = GaussianSystem {
      scale: 1.0
    };
    
    let r = init_ref_data(&system, &dictionary, &init_hl_slist);
    
    let mut w = init_working_data(&r);
    
    let mut the_winner: Option<HLSubstitution> = None;
    
    for _ in 0 .. 1000 {
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
    
    println!("");
    println!("Winner: {:?}", the_winner);
     
    let the_winner = the_winner.unwrap();
    
    let should = HLSubstitution {
      key: vec![T, H, E], at_start: false, at_end: false,
      sub_start: 0, sub_end: 3, sub_content: vec![Dh, Uh]
    };
    
    assert_eq!(the_winner, should);
  }
  
  #[test]
  fn gaussian_astarlike_test_2() {
    use crate::glyphs::Glyph::*;
    use crate::astarlike::*;
    use crate::high_level_substitutions::HLSubstitutionList;
    
    let mut dictionary = crate::dictionary::load_dictionary().unwrap();
    dictionary.words.truncate(20);
    let dictionary = dictionary;
    
    let init_hl_slist = HLSubstitutionList {
      substitutions: vec![]
    };
    
    let system = GaussianSystem {
      scale: 1.0
    };
    
    let r = init_ref_data(&system, &dictionary, &init_hl_slist);
    
    let mut w = init_working_data(&r);
    
    let mut the_winner: Option<HLSubstitution> = None;
    
    for _ in 0 .. 10000 {
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
    
    println!("");
    println!("Winner: {:?}", the_winner);
    
    let the_winner = the_winner.unwrap();
     
    let should = HLSubstitution {
      key: vec![T, H, E], at_start: false, at_end: false,
      sub_start: 0, sub_end: 3, sub_content: vec![Dh, Uh]
    };
    
    assert_eq!(the_winner, should);
  }
}


