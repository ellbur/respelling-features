
use feature_refining::astarlike::*;
use feature_refining::high_level_substitutions::*;
use feature_refining::dictionary;
use feature_refining::gaussian_astarlike2::*;
use howlong::SteadyTimer;

fn main() {
  let dictionary = dictionary::load_dictionary().unwrap();
  println!("Full size: {}", dictionary.words.len());
  

  let mut hl_slist = HLSubstitutionList::from_debug_str("").unwrap();
  
  let timer = SteadyTimer::new();
  'stepper_loop: for _i in 0 .. 10000 {
    // [th]→ϑ      [th]→ϑ     [th]→ϑ      [th]→ϑ     [th]→ϑ     [th]→ϑ   
    // [e]→ʌ       [i]→ɪ      [i]→ɪ       [i]→ɪ      [i]→ɪ      [i]→ɪ    
    // [i]→ɪ       [e]→ʌ      [e]→ʌ       [e]→ʌ      [e]→ʌ      [e]→ʌ    
    // ^[of]→ʌv    ^[of]→ʌv   [er]→ʳ      [er]→ʳ     [er]→ʳ     [er]→ʳ   
    // [a]→æ       [er]→ʳ     [a]→æ       [a]→æ      [a]→æ      [a]→æ    
    // [ou]→u      [a]→æ      ^[of]→ʌv    ^[of]→ʌv   ^[of]→ʌv   ^[of]→ʌv 
    // [s]$→z      [ou]→u     [ou]→u      [c]→k      [c]→k      [c]→k    
    // [er]→ʳ      [s]$→z     [s]$→z      [ng]→ŋ     [ng]→ŋ     [ng]→ŋ   
    // t[o]$→u     [c]→k      [c]→k       [ou]→u     [ou]→u     [ou]→u   
    // 1           2          3           4          5          6
    let system = GaussianSystem {
      scale: 6.0
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
  let duration = timer.elapsed();
  println!("{} {:?}", duration.as_millis(), hl_slist);
}

