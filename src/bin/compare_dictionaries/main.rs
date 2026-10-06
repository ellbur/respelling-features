
use feature_refining::astarlike::*;
use feature_refining::high_level_substitutions::*;
use feature_refining::dictionary;
use feature_refining::gaussian_astarlike2::*;

fn main() {
  let old_dictionary = dictionary::load_old_dictionary().unwrap();
  let new_dictionary = dictionary::load_dictionary().unwrap();

  for (name, dictionary) in [("old", &old_dictionary), ("new", &new_dictionary)] {
    println!("== {} ==", name);
    
    let mut hl_slist = HLSubstitutionList::from_debug_str("").unwrap();
    
    'stepper_loop: for _i in 0 .. 8 {
      let system = GaussianSystem {
        scale: 4.0
      };
      
      let r = init_ref_data(&system, &dictionary, &hl_slist);

      let mut w = init_working_data(&r);

      let winner: HLSubstitution;
      loop {
        match step(&system, &r, &mut w, false) {
          Some(outcome) => { 
            match outcome {
              Outcome::FoundImprovement(the_winner, _) => {
                winner = the_winner;
                break;
              }
              Outcome::FailedToFindImprovement(best, best_possible, before, after) => {
                println!("Failed to find improvement: {:?} {} {} {}", best, best_possible, before, after);
                break 'stepper_loop;
              }
            };
          },
          None => ()
        }
      }
      
      println!("{:?}", winner);
      
      hl_slist.substitutions.insert(0, winner);
    }
    
    println!("");
  }
}

