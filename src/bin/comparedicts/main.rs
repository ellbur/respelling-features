// Compares the phonemic dictionary with the CMUdict-based one word by word:
// the glyph substitutions, insertions and deletions in a minimal alignment,
// grouped and ranked by the frequency of the words they occur in, with
// examples. For finding systematic differences (e.g. British forms).
//
//   comparedicts [--top 60] [--words 5000]

use clap::Parser;
use std::collections::HashMap;

use feature_refining::glyphs::{Glyph, encode};

#[derive(Parser)]
struct Args {
  #[arg(long, default_value_t = 60)]
  top: usize,
  #[arg(long, default_value_t = 5000)]
  words: usize
}

fn edits(a: &[Glyph], b: &[Glyph]) -> Vec<(Option<Glyph>, Option<Glyph>)> {
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
      if a[i - 1] != b[j - 1] { res.push((Some(a[i - 1]), Some(b[j - 1]))); }
      i -= 1; j -= 1;
    }
    else if i > 0 && d[i][j] == d[i - 1][j] + 1 { res.push((Some(a[i - 1]), None)); i -= 1; }
    else { res.push((None, Some(b[j - 1]))); j -= 1; }
  }
  res
}

fn main() {
  let args = Args::parse();
  let mut ours = feature_refining::readlex::load_readlex_phonemic_dictionary().unwrap();
  ours.words.truncate(args.words);
  let cmu = feature_refining::dictionary::load_dictionary().unwrap();
  let cmu: HashMap<Vec<Glyph>, Vec<Glyph>> = cmu.words.into_iter().map(|w| (w.spelling, w.pronunciation)).collect();
  let max = ours.words.iter().map(|w| w.frequency).fold(0.0, f64::max);

  let mut groups: HashMap<(Option<Glyph>, Option<Glyph>), (f64, Vec<String>)> = HashMap::new();
  let mut compared = 0;
  for w in &ours.words {
    let Some(theirs) = cmu.get(&w.spelling) else { continue };
    compared += 1;
    for e in edits(&w.pronunciation, theirs) {
      let g = groups.entry(e).or_insert((0.0, vec![]));
      g.0 += w.frequency / max;
      if g.1.len() < 12 {
        g.1.push(format!("{}({}/{})", encode(&w.spelling), encode(&w.pronunciation), encode(theirs)));
      }
    }
  }
  let mut list: Vec<_> = groups.into_iter().collect();
  list.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap());
  println!("{} words compared (ours/CMU)", compared);
  let show = |g: Option<Glyph>| g.map_or("∅".to_owned(), |g| encode(&vec![g]));
  for ((a, b), (weight, examples)) in list.iter().take(args.top) {
    println!("{:>7.3}  {} → {}   {}", weight, show(*a), show(*b), examples.join(" "));
  }
}
