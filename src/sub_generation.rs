
use rand::{Rng, distributions::{WeightedIndex, Uniform}, prelude::Distribution};

use crate::{glyphs::Glyph, frequencies::FrequencyTable};
use crate::substitutions::Substitution;

pub struct SubGenerator {
  spelling_items: Vec<SpellingItem>,
  spelling_distribution: WeightedIndex<f64>
}

struct SpellingItem {
  spelling: Vec<Glyph>,
  sound_items: Vec<Glyph>,
  sound_distribution: WeightedIndex<f64>
}

impl SubGenerator {
  pub fn new(freq: FrequencyTable) -> SubGenerator {
    let mut spelling_items = vec![];
    let mut spelling_weights = vec![];
    
    for (spelling, entry) in freq.items.into_iter() {
      let mut sound_items = vec![];
      let mut sound_weights = vec![];
      
      for (sound, sound_frequency) in entry.associated_sound_frequencies.into_iter() {
        sound_items.push(sound);
        sound_weights.push(sound_frequency);
      }
      
      spelling_items.push(SpellingItem {
        spelling,
        sound_items,
        sound_distribution: WeightedIndex::new(sound_weights).unwrap()
      });
      spelling_weights.push(entry.frequency);
    }
    
    SubGenerator {
      spelling_items,
      spelling_distribution: WeightedIndex::new(spelling_weights).unwrap()
    }
  }
  
  pub fn next<R: Rng>(&self, rng: &mut R) -> Substitution {
    let spelling_item = &self.spelling_items[self.spelling_distribution.sample(rng)];
    let key = spelling_item.spelling.clone();
    let sub = spelling_item.sound_items[spelling_item.sound_distribution.sample(rng)].clone();
    
    let sub_len = Uniform::new(1, key.len()+1).sample(rng);
    let sub_start = Uniform::new(0, key.len()+1-sub_len).sample(rng);
    
    Substitution { key, sub_start, sub_end: sub_start + sub_len, sub_content: sub }
  }
}

