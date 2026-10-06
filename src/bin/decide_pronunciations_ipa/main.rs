#![allow(confusable_idents, uncommon_codepoints)]

use core::panic;

use feature_refining::ipa::{parse_ipa, Syllable};
use feature_refining::readlex::read_readlex_top_american5000;
use feature_refining::ipa::{Consonant::*, Vowel::*, Stress::*};
use std::{collections::HashSet, io::{self, BufRead, Write}, fs};
use itertools::Itertools;
use dialoguer::Select;
use console::style;

fn main() {
  let out_dict_path = "res/decided-pronunciations-ipa.txt";

  let existing: HashSet<String> = match fs::File::open(out_dict_path) {
    Ok(file) => io::BufReader::new(file).lines().map(|line| {
      line.unwrap().split(" ").next().unwrap().to_owned()
    }).collect(),
    Err(_) => HashSet::new()
  };

  let mut out_writer = io::BufWriter::new(fs::OpenOptions::new().create(true).append(true).open(out_dict_path).unwrap());

  let readlex = read_readlex_top_american5000().unwrap();

  let num_entries = readlex.len();
  for (entry_i, entry) in readlex.iter().enumerate() {
    if existing.contains(&entry.latin) {
      continue;
    }

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

    let parse_result = parse_ipa(&entry.ipa);

    match parse_result {
      Ok((rest, syllables)) => {
        if !rest.is_empty() {
          println!("Failed to parse: \"{}\" {} at {}", entry.latin, entry.ipa, rest);
          break;
        }

        let mut any_changed = false;

        let mut res: Vec<Syllable> = vec![];

        let multi_syllable = syllables.len() > 1;
        for (i, syllable) in syllables.iter().enumerate() {
          if multi_syllable && syllable.stress == Unstressed {
            if syllable.vowel == VI || syllable.vowel == Vɪ || syllable.vowel == Vi {
              let at_end = i == syllables.len() - 1;
              let is_ing = syllable.final_consonants == vec![Cŋ];
              if !(at_end && is_ing) {
                let mut prompt: String = format!("({}%) {}: ", entry_i*100/num_entries, entry.latin);
                for (j, other_syllable) in syllables.iter().enumerate() {
                  if i == j {
                    let (stress, initial, vowel, ffinal) = syllable.parts_to_ipa();
                    prompt.push_str(&stress);
                    prompt.push_str(&initial);
                    prompt.push_str(&style(vowel).bold().cyan().to_string());
                    prompt.push_str(&ffinal);
                  }
                  else {
                    prompt.push_str(&other_syllable.to_ipa());
                  }
                }

                let items: Vec<String> = vec![
                  "Schwa".to_string(),
                  "Ee".to_string(),
                  "Ih".to_string(),
                  "Eh".to_string()
                ];

                let replaced_vowel =
                  match Select::new().with_prompt(prompt).items(&items).interact().unwrap() {
                    0 => Və,
                    1 => Viː,
                    2 => Vɪ,
                    3 => Ve,
                    _ => panic!("No selection")
                  };

                res.push(Syllable { 
                  initial_consonants: syllable.initial_consonants.clone(),
                  vowel: replaced_vowel,
                  final_consonants: syllable.final_consonants.clone(),
                  stress: syllable.stress
                });
                any_changed = true;
                continue;
              }
            }
          }

          res.push(syllable.clone());
        }

        let encoded_res = res.iter().map(|s| s.to_ipa()).join("");

        if any_changed {
          out_writer.write(format!("{} {}\n", entry.latin, encoded_res).as_bytes()).unwrap();
          out_writer.flush().unwrap();
        }
      },
      Err(e) => {
        println!("Failed to parse: \"{}\" {}: {:?}", entry.latin, entry.ipa, e);
        break;
      }
    }
  }

  out_writer.flush().unwrap();
}

