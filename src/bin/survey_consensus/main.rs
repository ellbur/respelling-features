
use feature_refining::dictionary::load_dictionary;
use feature_refining::consensus_finding::build_justification_table;
use feature_refining::high_level_substitutions::HLSubstitution;
use float_ord::FloatOrd;

fn main() {
  let mut dictioary = load_dictionary().unwrap();
  
  println!("Loaded dictionary with {} words.", dictioary.words.len());
  
  dictioary.words.truncate(100);
  
  println!("Truncated to {} words.", dictioary.words.len());
  
  println!("Building justification table...");
  let jt = build_justification_table(&dictioary);
  println!("Done.");
  
  let mut edits: Vec<(HLSubstitution, f64)> = jt.by_sub.into_iter().collect();
  edits.sort_by_key(|(_, f)| FloatOrd(-*f));
  
  println!("Most popular edits:");
  for (edit, _) in edits.iter().take(30) {
    println!("{:?}", edit);
  }
}

