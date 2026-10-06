// Writes the OpenType features for a set of half-rules, and checks a font
// built with them.
//
//   makefont --rules <file> --features <out.fea> [--no-merge]
//     Merges the rules into as few lookups as the dictionary allows (see
//     lookups::merge) and writes the features: @lc, then an rlig feature
//     with a lowercasing lookup followed by the rules' lookups. Prints how
//     many token glyphs the font needs.
//
//   makefont --rules <file> --check-font <font.otf> [--no-merge]
//     Shapes every dictionary word with harfbuzz using the font and compares
//     with the simulation of the same lookups.
//
// The rules file is a pass-2 checkpoint or any file of half-rules, one per
// line (lines starting with # are ignored).

use clap::Parser;
use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use feature_refining::fea_parsing::render_fea_feature_body;
use feature_refining::glyphs::{AugGlyph, Glyph, aug_encode};
use feature_refining::half_rules::{HalfRule, ScoringWord};
use feature_refining::lookups::{self, PlainKind, plain_kind};
use feature_refining::second_pass::SecondPass;
use feature_refining::substitutions2 as s2;

#[derive(Parser)]
struct Args {
  #[arg(long)]
  rules: String,
  #[arg(long, default_value = "readlex")]
  dictionary: String,
  #[arg(long, default_value_t = 5000)]
  words: usize,
  // One lookup per half-rule instead of merging.
  #[arg(long)]
  no_merge: bool,
  #[arg(long)]
  features: Option<String>,
  #[arg(long)]
  check_font: Option<String>,
  // Mark word boundaries with a glyph instead of using ignore rules for
  // ^/$ (see features_with_boundaries).
  #[arg(long)]
  boundary_glyph: bool,
  // Also move later rules up into a lookup where that's safe (see
  // lookups::merge_reordering), looking this many rules ahead.
  #[arg(long)]
  reorder: Option<usize>,
  // Write context-free lookups of one kind as plain substitutions (see
  // render_groups), merging only rules of the same kind (see
  // lookups::merge_with).
  #[arg(long)]
  plain: bool,
  // End the feature with a lookup that changes nothing but marks every pair
  // of adjacent word glyphs unsafe to break (see features), so a browser
  // that reuses earlier shaping while text is typed reshapes whole words.
  #[arg(long)]
  glue: bool,
  // Finish by respelling each sound glyph in ASCII letters, as listed in
  // this file (see read_respelling), for a font that shows ordinary letters.
  #[arg(long)]
  respell: Option<String>
}

// A respelling file: lines of a glyph as the rules write it and its ASCII
// spelling ("ʌ uh"); # starts a comment.
fn read_respelling(path: &str) -> Vec<(AugGlyph, Vec<AugGlyph>)> {
  let text = fs::read_to_string(path).unwrap();
  text.lines().map(|l| l.split('#').next().unwrap().trim()).filter(|l| !l.is_empty()).map(|l| {
    let (from, to) = l.split_once(char::is_whitespace).unwrap();
    let from = feature_refining::glyphs::decode(from);
    assert_eq!(from.len(), 1, "{}: not one glyph", l);
    let to: Vec<AugGlyph> = feature_refining::glyphs::decode(to.trim()).into_iter().map(AugGlyph::Real).collect();
    (AugGlyph::Real(from[0]), to)
  }).collect()
}

// A word with each glyph respelled.
fn respelled(word: &[AugGlyph], respelling: &[(AugGlyph, Vec<AugGlyph>)]) -> Vec<AugGlyph> {
  word.iter().flat_map(|g| match respelling.iter().find(|(from, _)| from == g) {
    Some((_, to)) => to.clone(),
    None => vec![*g]
  }).collect()
}

// The respelling lookups: one-to-one and one-to-many substitutions can't
// share a lookup. Each lookup moves on past what it writes, but a later
// lookup sees it, so the one-to-many lookup goes first and the one-to-one
// lookup's inputs must not be letters (checked here): ɪ→i then i→ai would
// spell ɪ as "ai".
fn respell_lookups(respelling: &[(AugGlyph, Vec<AugGlyph>)]) -> String {
  let names = |gs: &[AugGlyph]| gs.iter().map(|g| g.name()).collect::<Vec<_>>().join(" ");
  let mut single = String::new();
  let mut multiple = String::new();
  let multiple_outputs: Vec<AugGlyph> = respelling.iter().filter(|(_, to)| to.len() > 1).flat_map(|(_, to)| to.clone()).collect();
  for (from, to) in respelling {
    assert!(to.len() > 1 || !multiple_outputs.contains(from), "{} is respelled one to one but also written by a one-to-many respelling", from.name());
    if to.len() == 1 {
      if to[0] != *from {
        let _ = writeln!(single, "    sub {} by {};", from.name(), names(to));
      }
    }
    else {
      let _ = writeln!(multiple, "    sub {} by {};", from.name(), names(to));
    }
  }
  format!("\n  lookup respell_multiple {{\n{}  }} respell_multiple;\n\n  lookup respell_single {{\n{}  }} respell_single;\n", multiple, single)
}

fn load_words(dictionary: &str, n: usize) -> Vec<ScoringWord> {
  let mut d = match dictionary {
    "cmudict" => feature_refining::dictionary::load_dictionary().unwrap(),
    "readlex" => feature_refining::readlex::load_readlex_ipa_american_tweaked_dictionary().unwrap(),
    "readlex-phonemic" => feature_refining::readlex::load_readlex_phonemic_dictionary().unwrap(),
    other => panic!("Unknown dictionary {}", other)
  };
  d.words.truncate(n);
  let max = d.words.iter().map(|w| w.frequency).fold(0.0, f64::max);
  d.words.iter().map(|w| ScoringWord {
    spelling: w.spelling.iter().map(|g| AugGlyph::Real(*g)).collect(),
    pronunciation: w.pronunciation.iter().map(|g| AugGlyph::Real(*g)).collect(),
    frequency: w.frequency / max
  }).collect()
}

fn max_token(rules: &[HalfRule]) -> Option<u32> {
  rules.iter()
    .flat_map(|r| r.pre.iter().flatten().chain(&r.at).chain(r.post.iter().flatten()).chain(&r.out))
    .filter_map(|g| match g { AugGlyph::Synthetic(n) => Some(*n), _ => None })
    .max()
}

// With `glue`, a last lookup substitutes each word glyph followed by
// another word glyph by itself. The output is unchanged, but HarfBuzz marks
// every such match unsafe to break, so the only safe breaks are at word
// starts. Browsers (Chrome, at least) reshape from the last safe break
// before an edit and reuse the shaping before it, which otherwise leaves
// stale results when a typed letter changes how earlier letters are spelled
// through lookahead.
fn features(groups: &[Vec<HalfRule>], num_tokens: u32, plain: bool, glue: bool, respell: Option<&[(AugGlyph, Vec<AugGlyph>)]>) -> String {
  // @lc: every glyph that counts as part of a word (see
  // Glyph::is_word_glyph), including every token.
  let mut lc: Vec<String> = Glyph::all().into_iter().filter(|g| g.is_word_glyph()).map(|g| g.name()).collect();
  lc.extend((0 .. num_tokens).map(|n| format!("syn{}", n)));

  let lowercase: String = ('a' ..= 'z').map(|c| format!("    sub {} by {};\n", c.to_ascii_uppercase(), c)).collect();
  let body = render_groups(groups, plain);
  let mut body: String = body.lines().map(|l| format!("  {}\n", l)).collect();
  if let Some(respelling) = respell {
    body += &respell_lookups(respelling);
  }

  let (identity, glue) = if glue {
    ("lookup identity {\n  sub @lc by @lc;\n} identity;\n\n", "\n  lookup glue {\n    sub @lc' lookup identity @lc;\n  } glue;\n")
  }
  else {
    ("", "")
  };

  format!(
    "# Generated by makefont.\n\n@lc = [{}];\n\n{}feature rlig {{\n  lookup lowercase {{\n{}  }} lowercase;\n\n{}{}}} rlig;\n",
    lc.join(" "), identity, lowercase, body, glue
  )
}

// The lookups for the rules. With `plain`, a lookup whose rules are all
// context-free and of one kind is written as that kind of substitution
// ("sub a b by c;") instead of as a contextual one ("sub a' b' by c;"), so
// feaLib compiles it into a single compact subtable instead of a chaining
// contextual lookup calling a nested one per rule.
fn render_groups(groups: &[Vec<HalfRule>], plain: bool) -> String {
  let mut out = String::new();
  for (i, group) in groups.iter().enumerate() {
    let kinds: Vec<Option<PlainKind>> = group.iter().map(plain_kind).collect();
    let plain_here = plain && kinds[0].is_some() && kinds.iter().all(|k| *k == kinds[0]);
    if plain_here {
      let _ = writeln!(out, "lookup l{} {{", i);
      for r in group {
        let names = |gs: &[AugGlyph]| gs.iter().map(|g| g.name()).collect::<Vec<_>>().join(" ");
        let _ = writeln!(out, "  sub {} by {};", names(&r.at), names(&r.out));
      }
      let _ = writeln!(out, "}} l{};", i);
    }
    else {
      let body = render_fea_feature_body(&lookups::low_level(&[group.clone()]));
      let _ = write!(out, "{}", body.replace("lookup l0 {", &format!("lookup l{} {{", i)).replace("} l0;", &format!("}} l{};", i)));
    }
  }
  out
}

// The word-boundary glyph (an empty glyph in the font).
const BOUNDARY: AugGlyph = AugGlyph::Synthetic(999);

// A rule whose ^ and $ are replaced by the boundary glyph in its context.
fn with_boundary_glyph(r: &HalfRule) -> HalfRule {
  let mut r = r.clone();
  if r.at_start {
    r.pre.insert(0, vec![BOUNDARY]);
    r.at_start = false;
  }
  if r.at_end {
    r.post.push(vec![BOUNDARY]);
    r.at_end = false;
  }
  r
}

fn wrap(word: &[AugGlyph]) -> Vec<AugGlyph> {
  let mut w = vec![BOUNDARY];
  w.extend_from_slice(word);
  w.push(BOUNDARY);
  w
}

fn unwrap(word: &[AugGlyph]) -> Vec<AugGlyph> {
  word.iter().filter(|g| **g != BOUNDARY).cloned().collect()
}

// Features that mark word boundaries with a glyph: after lowercasing, a
// boundary glyph is inserted before every letter that starts a word and after
// every letter that ends one (the only lookups that need a class of all
// letters); the rules match it in their contexts instead of using ignore
// rules; and at the end it's removed, which OpenType can only do by merging it
// into a neighbour with a ligature: first "wb x" -> x (word starts), then
// "x wb" -> x (word ends). They're separate lookups because in a one-letter
// word, "wb a wb", the first ligature would otherwise make the lookup skip the
// second boundary.
fn features_with_boundaries(groups: &[Vec<HalfRule>], num_tokens: u32, plain: bool) -> String {
  let wb = BOUNDARY.name();
  let letters: Vec<String> = ('a' ..= 'z').map(|c| c.to_string()).chain(["apos".to_owned()]).collect();
  // Every glyph that can be next to a boundary at the end.
  let mut neighbours: Vec<String> = Glyph::all().into_iter().filter(|g| g.is_word_glyph()).map(|g| g.name()).collect();
  neighbours.extend((0 .. num_tokens).map(|n| format!("syn{}", n)));
  
  let mut f = String::from("# Generated by makefont --boundary-glyph.\n\n");
  let _ = writeln!(f, "@letters = [{}];\n", letters.join(" "));
  let _ = writeln!(f, "lookup insert_before {{");
  for l in &letters { let _ = writeln!(f, "  sub {} by {} {};", l, wb, l); }
  let _ = writeln!(f, "}} insert_before;\n");
  let _ = writeln!(f, "lookup insert_after {{");
  for l in &letters { let _ = writeln!(f, "  sub {} by {} {};", l, l, wb); }
  let _ = writeln!(f, "}} insert_after;\n");
  
  let _ = writeln!(f, "feature rlig {{");
  let _ = writeln!(f, "  lookup lowercase {{");
  for c in 'a' ..= 'z' { let _ = writeln!(f, "    sub {} by {};", c.to_ascii_uppercase(), c); }
  let _ = writeln!(f, "  }} lowercase;\n");
  let _ = writeln!(f, "  lookup word_starts {{\n    ignore sub @letters @letters';\n    sub @letters' lookup insert_before;\n  }} word_starts;\n");
  let _ = writeln!(f, "  lookup word_ends {{\n    ignore sub @letters' @letters;\n    sub @letters' lookup insert_after;\n  }} word_ends;\n");
  for line in render_groups(groups, plain).lines() {
    let _ = writeln!(f, "  {}", line);
  }
  let _ = writeln!(f, "\n  lookup remove_starts {{");
  for g in &neighbours { let _ = writeln!(f, "    sub {} {} by {};", wb, g, g); }
  let _ = writeln!(f, "  }} remove_starts;\n");
  let _ = writeln!(f, "  lookup remove_ends {{");
  for g in &neighbours { let _ = writeln!(f, "    sub {} {} by {};", g, wb, g); }
  let _ = writeln!(f, "  }} remove_ends;");
  let _ = writeln!(f, "}} rlig;");
  f
}

fn main() {
  let args = Args::parse();
  let original_rules = SecondPass::load_checkpoint(&args.rules).expect("can't read the rules file").rules;
  let num_tokens = max_token(&original_rules).map_or(0, |n| n + 1);
  let original_words = load_words(&args.dictionary, args.words);
  
  // With a boundary glyph, the rules and the words they're merged on carry
  // boundary glyphs instead of ^/$.
  let (rules, words): (Vec<HalfRule>, Vec<ScoringWord>) = if args.boundary_glyph {
    (
      original_rules.iter().map(with_boundary_glyph).collect(),
      original_words.iter().map(|w| ScoringWord { spelling: wrap(&w.spelling), pronunciation: wrap(&w.pronunciation), frequency: w.frequency }).collect()
    )
  }
  else {
    (original_rules.clone(), original_words.iter().map(|w| ScoringWord { spelling: w.spelling.clone(), pronunciation: w.pronunciation.clone(), frequency: w.frequency }).collect())
  };
  
  let respelling: Option<Vec<(AugGlyph, Vec<AugGlyph>)>> = args.respell.as_deref().map(read_respelling);
  let groups: Vec<Vec<HalfRule>> = if args.no_merge {
    rules.iter().map(|r| vec![r.clone()]).collect()
  }
  else if let Some(window) = args.reorder {
    lookups::merge_reordering(&rules, &words, window)
  }
  else {
    lookups::merge_with(&rules, &words, args.plain)
  };
  println!("{} half-rules in {} lookups; the font needs syn0..syn{}{}", rules.len(), groups.len(), num_tokens.saturating_sub(1),
    if args.boundary_glyph { format!(" and {}", BOUNDARY.name()) } else { String::new() });
  
  if let Some(path) = &args.features {
    assert!(!((args.glue || respelling.is_some()) && args.boundary_glyph), "--glue and --respell aren't supported with --boundary-glyph");
    let text = if args.boundary_glyph { features_with_boundaries(&groups, num_tokens, args.plain) } else { features(&groups, num_tokens, args.plain, args.glue, respelling.as_deref()) };
    fs::write(path, text).unwrap();
    println!("Wrote {}", path);
  }

  if let Some(font) = &args.check_font {
    let slist = lookups::low_level(&groups);
    let text_file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
    let lines: Vec<String> = original_words.iter().map(|w| aug_encode(&w.spelling)).collect();
    fs::write(text_file.path(), lines.join("\n") + "\n").unwrap();
    let output = Command::new("hb-shape")
      .args([font.as_str(), &format!("--text-file={}", text_file.path().to_str().unwrap()), "--no-positions", "--no-clusters"])
      .output().unwrap();
    assert!(output.status.success(), "hb-shape failed");
    let shaped: Vec<&str> = std::str::from_utf8(&output.stdout).unwrap().lines().collect();
    assert_eq!(shaped.len(), words.len());
    
    let mut mismatches = 0;
    for (w, line) in original_words.iter().zip(shaped) {
      let by_hbshape: Vec<AugGlyph> = line.trim().trim_start_matches('[').trim_end_matches(']')
        .split('|').filter(|n| !n.is_empty()).map(|n| AugGlyph::from_name(n).unwrap_or_else(|| panic!("unknown glyph {}", n))).collect();
      let by_sim = if args.boundary_glyph {
        let mut wrapped = wrap(&w.spelling);
        s2::apply_all(&mut wrapped, &slist);
        unwrap(&wrapped)
      }
      else {
        let mut by_sim = w.spelling.clone();
        s2::apply_all(&mut by_sim, &slist);
        match &respelling {
          Some(respelling) => respelled(&by_sim, respelling),
          None => by_sim
        }
      };
      if by_hbshape != by_sim {
        mismatches += 1;
        if mismatches <= 10 {
          println!("  {}: harfbuzz {} simulation {}", aug_encode(&w.spelling), aug_encode(&by_hbshape), aug_encode(&by_sim));
        }
      }
    }
    println!("{} of {} words shape differently from the simulation", mismatches, words.len());
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use feature_refining::half_rules::apply_all_copied;
  
  // Rules with boundary glyphs, on words wrapped in them, must do exactly what
  // the anchored rules do.
  #[test]
  fn boundary_glyph_rules_match_anchors() {
    let rules = feature_refining::half_rules::flatten(&feature_refining::high_level_substitutions2::HLSubstitutionList::set_1());
    assert!(rules.iter().any(|r| r.at_start || r.at_end));
    let transformed: Vec<HalfRule> = rules.iter().map(with_boundary_glyph).collect();
    for w in load_words("cmudict", 3000) {
      assert_eq!(unwrap(&apply_all_copied(&transformed, &wrap(&w.spelling))), apply_all_copied(&rules, &w.spelling), "{}", aug_encode(&w.spelling));
    }
  }
}
