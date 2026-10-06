
use feature_refining::high_level_substitutions2::HLSubstitutionList;
use feature_refining::dictionary::{load_dictionary, DictionaryWord};
use feature_refining::astarlike2::distanceg;
use feature_refining::glyphs::{strip_aug, Glyph, encode, augment};
use float_ord::FloatOrd;

struct ProError<'d> {
  word: &'d DictionaryWord,
  transformed_spelling: Vec<Glyph>,
  weighted_error: f64
}
  
fn main() {
  let hlist = HLSubstitutionList::set_1();
    
  let dictionary = load_dictionary().unwrap();
  
  let mut errors: Vec<ProError> = dictionary.words.iter().map(|word| {
    let transformed_spelling = strip_aug(&hlist.apply_copied_always(&augment(&word.spelling)));
    let error = distanceg(&transformed_spelling, &word.pronunciation);
    let weighted_error = word.frequency * (error as f64);
    
    ProError {
      word,
      weighted_error,
      transformed_spelling
    }
  }).collect();
  
  errors.sort_by_key(|e| FloatOrd(-e.weighted_error));
  
  for e in errors.iter().take(20) {
    println!("{} -> {} vs {}", encode(&e.word.spelling), encode(&e.transformed_spelling), encode(&e.word.pronunciation));
  }
}

