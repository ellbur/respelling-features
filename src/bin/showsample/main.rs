// Shows a text as a set of half-rules spells it, word by word, marking
// errors against the dictionary in red (console escape codes): wrong or
// extra glyphs in red, a missing glyph as a red underscore. Words not in the
// dictionary are shown dimmed, and tokens left in the output in red braces.
//
//   showsample --rules <file> [--dictionary readlex-phonemic] < text

use clap::Parser;
use std::collections::HashMap;
use std::io::Read;

use feature_refining::glyphs::{AugGlyph, Glyph, aug_encode, decode};
use feature_refining::half_rules::apply_all_copied;
use feature_refining::second_pass::SecondPass;

#[derive(Parser)]
struct Args {
  #[arg(long)]
  rules: String,
  #[arg(long, default_value = "readlex-phonemic")]
  dictionary: String,
  #[arg(long, default_value_t = 5000)]
  words: usize
}

const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

// The output with the glyphs that a minimal Levenshtein alignment against
// `want` marks as wrong in red.
fn colored(got: &[AugGlyph], want: &[AugGlyph]) -> String {
  let (n, m) = (got.len(), want.len());
  let mut d = vec![vec![0u32; m + 1]; n + 1];
  for i in 0 ..= n { d[i][0] = i as u32; }
  for j in 0 ..= m { d[0][j] = j as u32; }
  for i in 1 ..= n {
    for j in 1 ..= m {
      let sub = d[i - 1][j - 1] + if got[i - 1] == want[j - 1] { 0 } else { 1 };
      d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
    }
  }
  let show = |g: AugGlyph| aug_encode(&vec![g]);
  let mut parts = vec![];
  let (mut i, mut j) = (n, m);
  while i > 0 || j > 0 {
    if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + if got[i - 1] == want[j - 1] { 0 } else { 1 } {
      parts.push(if got[i - 1] == want[j - 1] { show(got[i - 1]) } else { format!("{}{}{}", RED, show(got[i - 1]), RESET) });
      i -= 1;
      j -= 1;
    }
    else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
      parts.push(format!("{}{}{}", RED, show(got[i - 1]), RESET));
      i -= 1;
    }
    else {
      parts.push(format!("{}_{}", RED, RESET));
      j -= 1;
    }
  }
  parts.reverse();
  parts.concat()
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
  let pronunciations: HashMap<Vec<Glyph>, Vec<Glyph>> = dictionary.words.iter()
    .map(|w| (w.spelling.clone(), w.pronunciation.clone())).collect();

  let mut text = String::new();
  std::io::stdin().read_to_string(&mut text).unwrap();
  let (mut known, mut wrong, mut unknown) = (0, 0, 0);
  for line in text.lines() {
    let mut out = vec![];
    for token in line.split_whitespace() {
      let lower = token.to_lowercase();
      // Leading and trailing punctuation stays as it is.
      let word = lower.trim_matches(|c: char| !c.is_alphabetic());
      let start = lower.find(word).unwrap_or(0);
      let (before, after) = (&lower[.. start], &lower[start + word.len() ..]);
      let spelling = decode(word);
      if spelling.is_empty() {
        out.push(token.to_owned());
        continue;
      }
      let input: Vec<AugGlyph> = spelling.iter().map(|g| AugGlyph::Real(*g)).collect();
      let got = apply_all_copied(&rules, &input);
      let shown = match pronunciations.get(&spelling) {
        Some(p) => {
          known += 1;
          let want: Vec<AugGlyph> = p.iter().map(|g| AugGlyph::Real(*g)).collect();
          if got != want {
            wrong += 1;
          }
          colored(&got, &want)
        },
        None => {
          unknown += 1;
          format!("{}{}{}", DIM, aug_encode(&got), RESET)
        }
      };
      out.push(format!("{}{}{}", before, shown, after));
    }
    println!("{}", out.join(" "));
  }
  eprintln!("{} words in the dictionary, {} of them wrong; {} not in the dictionary (dimmed)", known, wrong, unknown);
}
