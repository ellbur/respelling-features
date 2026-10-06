
use feature_refining::astarlike2::*;
use feature_refining::high_level_substitutions2::*;
use feature_refining::dictionary;
use feature_refining::gaussian_astarlike22::*;

fn main() {
  let dictionary = dictionary::load_dictionary().unwrap();
  println!("Full size: {}", dictionary.words.len());
  
  let init_rules = HLSubstitutionList {
    substitutions: vec![]
  };
  
  // 4.0              5.0
  // [th]→0→ϑ         [th]→0→ϑ      
  // [i]→1→ɪ          [i]→1→ɪ       
  // [a]→2→æ          [a]→2→æ       
  // [e]→3→ʌ          [e]→3→ʌ       
  // [{3}r]→4→ʳ       [{3}r]→4→ʳ    
  // ^[of]→5→{3}v     ^[of]→5→{3}v  
  // [ng]→6→ŋ         [c]→6→k       
  // [s]$→7→z         [ng]→7→ŋ      
  // [c]→8→k          [s]$→8→z      
  // [ou]→9→u         [ou]→9→u      
  // [{3}{2}]→10→ɩ    [{3}{2}]→10→ɩ 
  // [to]→11→t{9}     [to]→11→t{9}  
  // [y]$→12→{10}     [y]$→12→{10}  
  // [o]r→13→ɔ        [o]r→13→ɔ     
  // [{2}]$→14→a      [{2}]$→14→a   
  // [{8}h]→15→ʧ      [{6}h]→15→ʧ   

  let system = GaussianSystem {
    scale: 4.0
  };
  
  let mut iter_system = feature_refining::astarlike2::IterativeSystem::setup(&dictionary, init_rules);
  
  'stepper_loop: for _i in 0 .. 400 {
    match iter_system.find_next_rule(&system, false) {
      Outcome::FoundImprovement(the_winner, _) => {
        println!("{}", the_winner.encode());
      }
      Outcome::FailedToFindImprovement(best, change) => {
        println!("Failed to find improvement: {} {}", best.encode(), change);
        break 'stepper_loop;
      }
    };
  }
}

