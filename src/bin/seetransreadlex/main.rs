
use feature_refining::readlex::{read_readlex_top5000, shaw_word_to_glyphs_with_fixes};
use rand::{distributions::Uniform, prelude::Distribution};
use feature_refining::glyphs::encode;

fn main() {
  let all_words = read_readlex_top5000().unwrap();

  let mut rng = rand::thread_rng();
  let all_words_dist = Uniform::new(0, all_words.len());

  for _ in 0 .. 5 {
    let i = all_words_dist.sample(&mut rng);
    let word = &all_words[i];
    let latin = &word.latin;
    let shaw = &word.shaw;
    let shaw_glyphs = shaw_word_to_glyphs_with_fixes(shaw, latin);
    let shaw_pretty = encode(&shaw_glyphs);

    println!("{:?} {:?} {:?} {:?}", latin, shaw, shaw_glyphs, shaw_pretty);
  }
}

