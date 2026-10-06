
use feature_refining::ipa::parse_ipa;
use feature_refining::readlex::read_readlex_top5000;

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
      Ok((rest, _)) => {
        if !rest.is_empty() {
          println!("Failed to parse: \"{}\" {} at {}", entry.latin, entry.ipa, rest);
          break;
        }
      },
      Err(e) => {
        println!("Failed to parse: \"{}\" {}: {:?}", entry.latin, entry.ipa, e);
        break;
      }
    }
  }
}

