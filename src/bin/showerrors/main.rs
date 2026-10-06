// Shows where a set of half-rules gets the dictionary wrong: the words that
// contribute the most frequency-weighted error, and the most common kinds of
// error (a sound where another should be, a missing sound, an extra one)
// across all words.
//
//   showerrors --rules <file> [--top 20]

use clap::Parser;
use std::collections::HashMap;

use feature_refining::glyphs::{AugGlyph, aug_encode};
use feature_refining::half_rules::apply_all_copied;
use feature_refining::second_pass::SecondPass;

#[derive(Parser)]
struct Args {
  #[arg(long)]
  rules: String,
  #[arg(long, default_value_t = 5000)]
  words: usize,
  #[arg(long, default_value_t = 20)]
  top: usize,
  #[arg(long, default_value = "readlex")]
  dictionary: String
}

// The edits turning `a` into `b` in a minimal Levenshtein alignment, as
// (from, to) pairs: (Some(x), Some(y)) for x where y should be, (Some(x),
// None) for an extra x, (None, Some(y)) for a missing y.
fn edits(a: &[AugGlyph], b: &[AugGlyph]) -> Vec<(Option<AugGlyph>, Option<AugGlyph>)> {
  let (n, m) = (a.len(), b.len());
  let mut d = vec![vec![0u32; m + 1]; n + 1];
  for i in 0 ..= n { d[i][0] = i as u32; }
  for j in 0 ..= m { d[0][j] = j as u32; }
  for i in 1 ..= n {
    for j in 1 ..= m {
      let sub = d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 };
      d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
    }
  }
  let mut res = vec![];
  let (mut i, mut j) = (n, m);
  while i > 0 || j > 0 {
    if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 } {
      if a[i - 1] != b[j - 1] {
        res.push((Some(a[i - 1]), Some(b[j - 1])));
      }
      i -= 1;
      j -= 1;
    }
    else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
      res.push((Some(a[i - 1]), None));
      i -= 1;
    }
    else {
      res.push((None, Some(b[j - 1])));
      j -= 1;
    }
  }
  res
}

fn show(g: Option<AugGlyph>) -> String {
  g.map_or("∅".to_owned(), |g| aug_encode(&vec![g]))
}

fn main() {
  let args = Args::parse();
  let rules = SecondPass::load_checkpoint(&args.rules).expect("can't read the rules file").rules;

  let mut dictionary = match args.dictionary.as_str() {
    "readlex" => feature_refining::readlex::load_readlex_ipa_american_tweaked_dictionary().unwrap(),
    "readlex-phonemic" => feature_refining::readlex::load_readlex_phonemic_dictionary().unwrap(),
    other => panic!("Unknown dictionary {}", other)
  };
  dictionary.words.truncate(args.words);
  let max = dictionary.words.iter().map(|w| w.frequency).fold(0.0, f64::max);

  let mut word_errors = vec![];
  let mut edit_weights: HashMap<(Option<AugGlyph>, Option<AugGlyph>), (f64, usize)> = HashMap::new();
  let mut total = 0.0;
  let mut wrong = 0;

  for w in &dictionary.words {
    let spelling: Vec<AugGlyph> = w.spelling.iter().map(|g| AugGlyph::Real(*g)).collect();
    let pronunciation: Vec<AugGlyph> = w.pronunciation.iter().map(|g| AugGlyph::Real(*g)).collect();
    let output = apply_all_copied(&rules, &spelling);
    let frequency = w.frequency / max;
    let es = edits(&output, &pronunciation);
    if es.is_empty() {
      continue;
    }
    wrong += 1;
    let error = frequency * es.len() as f64;
    total += error;
    word_errors.push((error, aug_encode(&spelling), aug_encode(&output), aug_encode(&pronunciation)));
    for e in es {
      let entry = edit_weights.entry(e).or_insert((0.0, 0));
      entry.0 += frequency;
      entry.1 += 1;
    }
  }

  println!("{} of {} words wrong; total weighted error {:.4}", wrong, dictionary.words.len(), total);

  word_errors.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
  println!("\nWords contributing the most error:");
  println!("  {:>6}  {:14} {:14} {:14}", "share", "spelling", "computed", "actual");
  for (error, spelling, output, pronunciation) in word_errors.iter().take(args.top) {
    println!("  {:>5.1}%  {:14} {:14} {:14}", 100.0 * error / total, spelling, output, pronunciation);
  }

  let mut edit_list: Vec<_> = edit_weights.into_iter().collect();
  edit_list.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap());
  println!("\nMost common errors (computed -> actual), across all words:");
  println!("  {:>6}  {:>5}  {}", "share", "words", "error");
  for ((from, to), (weight, count)) in edit_list.iter().take(args.top) {
    let what = match (from, to) {
      (Some(_), Some(_)) => format!("{} where {} should be", show(*from), show(*to)),
      (Some(_), None) => format!("extra {}", show(*from)),
      _ => format!("missing {}", show(*to))
    };
    println!("  {:>5.1}%  {:>5}  {}", 100.0 * weight / total, count, what);
  }
}
