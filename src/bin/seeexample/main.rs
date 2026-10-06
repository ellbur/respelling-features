
use feature_refining::high_level_substitutions2::HLSubstitutionList;
use feature_refining::dictionary::load_dictionary;
use feature_refining::glyphs::{strip_aug, Glyph, encode, augment, decode};
use clap::Parser;
use regex::Regex;
use std::collections::HashMap;
use console::style;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg()]
    sentence: String,
}

fn annotate_errors(is: &Vec<Glyph>, should: &Vec<Glyph>) -> Vec<bool> {
  let n = is.len();
  let mut res = vec![false; n];
    
  let mut k = 0;
  for item in diff::slice(&is, &should) {
    match item {
      diff::Result::Left(_) => {
        res[k] = true;
        k += 1;
      },
      diff::Result::Both(_, _) => {
        k += 1;
      },
      diff::Result::Right(_) => { }
    }
  }
    
  res
}
  
fn main() {
  let args = Args::parse();
  
  let hlist = HLSubstitutionList::set_1();
    
  let dictionary = load_dictionary().unwrap();
  
  let dictionary: HashMap<Vec<Glyph>, Vec<Glyph>> = dictionary.words.into_iter().map(|w| {
    (w.spelling, w.pronunciation)
  }).collect();
  
  let pre_stripping_re = Regex::new(r"^[\.\^\:\;\,\?]*").unwrap();
  let post_stripping_re = Regex::new(r"[\.\^\:\;\,\?]$").unwrap();
  
  for word in args.sentence.split(" ") {
    let word = word.trim();
    let word = pre_stripping_re.replace_all(word, "");
    let word = post_stripping_re.replace_all(&word, "");
    
    let spelling = decode(&word);
    let transformed_spelling = strip_aug(&hlist.apply_copied_always(&augment(&spelling)));
      
    match dictionary.get(&spelling) {
      None => {
        print!("{}", style(encode(&transformed_spelling)).dim());
      },
      Some(pronunciation) => {
        let errors = annotate_errors(&transformed_spelling, pronunciation);
        
        for i in 0 .. transformed_spelling.len() {
          let c = encode(&vec![transformed_spelling[i]]);
          if errors[i] {
            print!("{}", style(c).red());
          }
          else {
            print!("{}", c);
          }
        }
      }
    }
    print!(" ");
  }
  println!("\n");
}

