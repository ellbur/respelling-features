
use feature_refining::{ipa::parse_ipa, readlex::read_readlex_top5000};
use itertools::Itertools;
use console::style;
use feature_refining::ipa::Stress::Unstressed;

fn main() {
  let readlex = read_readlex_top5000().unwrap();

  for entry in readlex {
    if   entry.latin == "the" 
      || entry.latin == "of" 
      || entry.latin == "and" 
      || entry.latin == "to" 
      || entry.latin == "for" 
      || entry.latin == "mm" 
      || entry.latin == "etc"
    {
      continue;
    }

    if entry.latin.contains(" ") || entry.latin.contains("-") {
      continue;
    }
    
    let parse_result = parse_ipa(&entry.ipa);

    match parse_result {
      Ok((rest, syllables)) => {
        if !rest.is_empty() {
          println!("Failed to parse: \"{}\" {} at {}", entry.latin, entry.ipa, rest);
          break;
        }

        for i in 0 .. syllables.len()-1 {
          if syllables[i].final_consonants.is_empty() && syllables[i+1].initial_consonants.is_empty() && syllables[i+1].stress == Unstressed {
            let (stress_1, initial_1, vowel_1, final_1) = syllables[i].parts_to_ipa();
            let (stress_2, initial_2, vowel_2, final_2) = syllables[i+1].parts_to_ipa();
            println!("{}: {}{}{}{} {}{}{}{}{}{}",
              entry.latin,
              syllables[0 .. i].iter().map(|s| s.to_ipa()).join(""),
              stress_1,
              initial_1,
              style(vowel_1).bold().cyan(),
              style(final_1).bold().cyan(),
              style(stress_2).bold().cyan(),
              style(initial_2).bold().cyan(),
              style(vowel_2).bold().cyan(),
              final_2,
              syllables[i+2 ..].iter().map(|s| s.to_ipa()).join("")
            );
          }
        }
      },
      Err(e) => {
        println!("Failed to parse: \"{}\" {}: {:?}", entry.latin, entry.ipa, e);
        break;
      }
    }
  }
}

