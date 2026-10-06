
// vim: shiftwidth=2

use std::{vec::Vec, fs, path::Path};
use float_ord::FloatOrd;
use rand::{Rng, distributions::{Uniform, Bernoulli, WeightedIndex}, prelude::Distribution, thread_rng, rngs::ThreadRng};
use lazy_static::lazy_static;
use levenshtein_diff as levenshtein;

use feature_refining::glyphs::*;
use feature_refining::dictionary::*;
use serde::{Serialize, Deserialize};
use feature_refining::sub_generation::SubGenerator;
use feature_refining::substitutions::*;
use feature_refining::frequencies;

fn score_difference(p1: &Vec<Glyph>, p2: &Vec<Glyph>) -> f64 {
  levenshtein::distance(p1, p2).0 as f64
}

fn score_dictionary_transformer<F: Fn(&Vec<Glyph>) -> Vec<Glyph>>(dictionary: &Dictionary, transformer: F) -> f64 {
  let mut total: f64 = 0.0;
  
  for word in &dictionary.words {
    let transformed = transformer(&word.spelling);
    let score = score_difference(&word.pronunciation, &transformed) as f64;
    total = total + score * (word.frequency as f64);
  }
  
  total
}

fn score_substitution_list(dictionary: &Dictionary, substitution_list: &SubstitutionList) -> f64 {
  score_dictionary_transformer(dictionary, |word| {
    let mut word = word.clone();
    substitution_list.apply_all_pos(&mut word);
    word
  })
}

fn base_score(dictionary: &Dictionary) -> f64 {
  let mut total: f64 = 0.0;
  
  for word in &dictionary.words {
    let score = score_difference(&word.pronunciation, &word.spelling) as f64;
    total = total + score * (word.frequency as f64);
  }
  
  total as f64
}

#[cfg(test)]
mod scoring_sanity_tests {
  use super::*;

  #[test]
  fn test_1() {
    use Glyph::*;
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: vec![N, I, G, H, T],
          pronunciation: vec![N, I, T],
          frequency: 1.0
        },
        DictionaryWord {
          spelling: vec![C, R, A, T, E],
          pronunciation: vec![K, R, Ei, T],
          frequency: 1.0
        },
      ]
    };
    let substitution_list = SubstitutionList {
      substitutions: vec![
        SubstitutionListItem::Substitution(Substitution {
          key: vec![I, G, H], sub_start: 0, sub_end: 3, sub_content: I
        }),
        SubstitutionListItem::Substitution(Substitution {
          key: vec![A, T, E], sub_start: 0, sub_end: 1, sub_content: Ei
        }),
        SubstitutionListItem::Barrier,
        SubstitutionListItem::Substitution(Substitution {
          key: vec![Ei, T, E], sub_start: 1, sub_end: 3, sub_content: T
        }),
      ]
    };
    let score1 = base_score(&dictionary);
    let score2 = score_substitution_list(&dictionary, &substitution_list);
    assert!(score2 < score1);
  }
}

fn mutate<F2: Fn(&mut ThreadRng) -> Substitution>(rng: &mut ThreadRng, sl: &mut SubstitutionList, sub_gen: F2) {
  lazy_static! {
    static ref BARRIER_LIKELIHOOD: Bernoulli = Bernoulli::new(0.05).unwrap();
  }
  
  let deletion_likelihood: Bernoulli = Bernoulli::new(0.3).unwrap();
  let insertion_likelihood: Bernoulli = Bernoulli::new(0.3).unwrap();
  let transposition_likelihood: Bernoulli = Bernoulli::new(0.1).unwrap();
  
  let mut did_mod = false;
  
  while sl.substitutions.len() > 1 {
    if !deletion_likelihood.sample(rng) {
      break;
    }
    did_mod = true;
    sl.substitutions.remove(Uniform::new(0, sl.substitutions.len()).sample(rng));
  }
  
  while sl.substitutions.len() < 500 {
    if did_mod && !insertion_likelihood.sample(rng) {
      break;
    }
    did_mod = true;
    sl.substitutions.insert(Uniform::new(0, sl.substitutions.len()+1).sample(rng), {
      if BARRIER_LIKELIHOOD.sample(rng) {
        SubstitutionListItem::Barrier
      }
      else {
        SubstitutionListItem::Substitution(sub_gen(rng))
      }
    });
  }
  
  loop {
    if !transposition_likelihood.sample(rng) {
      break;
    }
    let i1 = Uniform::new(0, sl.substitutions.len()).sample(rng);
    let i2 = Uniform::new(0, sl.substitutions.len()).sample(rng);
    sl.substitutions.swap(i1, i2);
  }
}

fn combine<R: Rng>(rng: &mut R, sl1: &SubstitutionList, sl2: &SubstitutionList) -> SubstitutionList {
  if sl1.substitutions.len() == 0 {
    return sl2.clone();
  }
  if sl2.substitutions.len() == 0 {
    return sl1.clone();
  }
  
  let len1 = sl1.substitutions.len();
  let len2 = sl2.substitutions.len();
  let total_len = len1 + len2;
  
  let bias = (len1 as f64) / (total_len as f64);
  let bias_dist = Bernoulli::new(bias).unwrap();
  
  let r21 = (len2 as f64) / (len1 as f64);
  let r12 = (len1 as f64) / (len2 as f64);
  
  let mut f1: f64 = 0.0;
  let mut f2: f64 = 0.0;
  
  let mut res = SubstitutionList { substitutions: vec![] };
  
  loop {
    let i1 = f1 as usize;
    let i2 = f2 as usize;
    
    if i1 >= len1 && i2 >= len2 {
      break;
    }
    
    let which = {
      if i1 >= len1 {
        2
      }
      else if i2 >= len2 {
        1
      }
      else {
        if bias_dist.sample(rng) {
          1
        }
        else {
          2
        }
      }
    };
    
    if which == 1 {
      res.substitutions.push(sl1.substitutions[i1].clone());
      f1 += 1.0;
      f2 += r21;
    }
    else {
      res.substitutions.push(sl2.substitutions[i2].clone());// Here
      f2 += 1.0;
      f1 += r12;
    }
  }
  
  res
}

#[derive(Debug, Serialize, Deserialize)]
struct PopulationItem {
  sl: SubstitutionList,
  score: f64
}

#[derive(Debug, Serialize, Deserialize)]
struct Population {
  items: Vec<PopulationItem>
}

impl Population {
  fn new<F: Fn(&SubstitutionList) -> f64>(scorer: F) -> Population {
    let sl = SubstitutionList { substitutions: vec![] };
    let score = scorer(&sl);
    Population { items: vec![PopulationItem { sl, score }] }
  }
  
  fn expand<F: Fn(&SubstitutionList) -> f64 + Send + Sync, F2: Fn(&mut ThreadRng) -> Substitution + Send + Sync>(&mut self, target_size: usize, scorer: F, sub_gen: F2) {
    use rayon::prelude::*;
    
    lazy_static! {
      static ref COMBINE_LIKELIHOOD: Bernoulli = Bernoulli::new(0.75).unwrap();
    }
    
    let start_size = self.items.len();
    let sampling_dist = WeightedIndex::new(self.items.iter().map(|it| 1.0 / it.score)).unwrap();
    
    if target_size > start_size {
      let new_items: Vec<PopulationItem> = (0 .. (target_size - start_size)).into_par_iter().map(|_| {
        let rng = &mut thread_rng();
        let c = {
          if COMBINE_LIKELIHOOD.sample(rng) {
            let sl1 = &self.items[sampling_dist.sample(rng)].sl;
            let sl2 = &self.items[sampling_dist.sample(rng)].sl;
            let mut c = combine(rng, sl1, sl2);
            mutate(rng, &mut c, &sub_gen);
            c
          }
          else {
            let sl = &self.items[sampling_dist.sample(rng)].sl;
            let mut c = sl.clone();
            mutate(rng, &mut c, &sub_gen);
            c
          }
        };
        let score = scorer(&c);
        PopulationItem { sl: c, score }
      }).collect();
      self.items.extend(new_items);
    }
  }
  
  fn cull(&mut self, target_size: usize) {
    if self.items.len() > target_size {
      self.items.sort_by_key(|it| FloatOrd(it.score));
      self.items.truncate(target_size);
    }
  }
}

struct ScoringSetup {
  sub_gen: SubGenerator,
  dictionary: Dictionary,
  length_penalty_a: f64,
  length_penalty_b: f64
}

impl ScoringSetup {
  fn score(&self, s: &SubstitutionList) -> f64 {
    let length_score = self.length_penalty_a * ((s.substitutions.len() as f64)*self.length_penalty_b).exp();
    score_substitution_list(&self.dictionary, s) + length_score
  }
  
  fn make_sub(&self, rng: &mut ThreadRng) -> Substitution {
    self.sub_gen.next(rng)
  }
}

fn refine(setup: &ScoringSetup, population: &mut Population, num_iters: usize) {
  let mut staleness = 0;
  for i in 0..num_iters{
    println!("Loop {}/{}", i, num_iters);
    
    let old_best_score = population.items[0].score;
    
    let expand_size = {
      if staleness <= 3 {
        100
      }
      else {
        100 + staleness*100
      }
    };
    
    let cull_size = expand_size / 2;
    
    population.expand(expand_size, |s| setup.score(s), |rng| setup.make_sub(rng));
    population.cull(cull_size);
    
    let new_best_score = population.items[0].score;
    if old_best_score == new_best_score {
      if staleness <= 100 {
        staleness += 1;
      }
    }
    else {
      staleness = 0;
    }
    
    println!("Best at {}", new_best_score);
  }
}

fn run() {
  let freq_table = frequencies::load();
  let sub_gen = SubGenerator::new(freq_table);
  let dictionary = load_dictionary().unwrap();
  let base_score = base_score(&dictionary);
  
  let length_penalty_b = 1.0/500.0;
  let length_penalty_a = base_score / (1.0 as f64).exp();
  
  let setup = ScoringSetup {
    sub_gen,
    dictionary,
    length_penalty_a,
    length_penalty_b
  };
  
  let cache_path = "cache.json";
  let backup_path = "cache-backup.json";
  
  let mut population: Population = {
    if Path::new(cache_path).exists() {
      serde_json::from_reader(fs::File::open(cache_path).unwrap()).unwrap()
    }
    else {
      Population::new(|s| setup.score(s))
    }
  };
  
  loop {
    refine(&setup, &mut population, 30);
    population.cull(10);
    
    if Path::new(cache_path).exists() {
      fs::copy(cache_path, backup_path).unwrap();
    }
    serde_json::to_writer(fs::File::create(cache_path).unwrap(), &population).unwrap();
    
    population.cull(1);
    println!("{}", population.items[0].sl.render());
  }
}

fn main() {
  run();
}

