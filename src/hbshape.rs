
use std::{io::{self}, path::Path, process::{Command, ExitStatus}, str::from_utf8, fs, collections::HashSet, sync::OnceLock};

use lazy_static::lazy_static;
use regex::Regex;
use tempfile::{Builder, NamedTempFile};

use crate::{substitutions2::*, glyphs::AugGlyph};
use crate::fea_parsing as p;

// Made by scripts/make_ascii_base.py res/test-font.ttf --syn 100 --family "Respelling Test".
const TEST_FONT_PATH: &str = "res/test-font.ttf";

pub fn apply_using_hbshape(slist: &SubstitutionList, text: &Vec<AugGlyph>) -> io::Result<Vec<AugGlyph>> {
  // The input goes to hb-shape as Unicode text, so it can only contain glyphs
  // the font maps from characters. Tokens have to be produced by the lookups.
  for g in text {
    match g {
      AugGlyph::Real(g) if g.char().is_ascii() => (),
      _ => panic!("hb-shape input must be ASCII letters or punctuation, got {:?}", g)
    }
  }
  let encoded_text = &crate::glyphs::aug_encode(text);
  
  let fea_file = Builder::new().suffix(".fea").tempfile()?;
  let in_otf_path = Path::new(TEST_FONT_PATH);
  let out_otf_file = Builder::new().suffix(".ttf").tempfile()?;
  
  let text = format!("
{}
  
feature rlig {{
  {}
}} rlig;
", p::render_lc_class(test_font_glyph_names()?), p::render_fea_feature_body(slist));
  
  fs::write(&fea_file, text)?;
  
  // fonttools feaLib -o with-feats.ttf features.fea res/test-font.ttf
  successful(Command::new("fonttools").args(["feaLib", "-o", p(&out_otf_file), p(&fea_file), p2(in_otf_path)]).status()?, "fonttools")?;
  
  // hb-shape with-feats.ttf 'you'
  let output = Command::new("hb-shape").args([p(&out_otf_file), encoded_text]).output()?;
  successful(output.status, "hb-shape")?;
  
  let output_text = from_utf8(&output.stdout).unwrap();
  parse_shaping_output(output_text)
}

// apply_using_hbshape for many texts, compiling the font once. Each text is
// shaped on its own (one per line of hb-shape's input).
pub fn apply_many_using_hbshape(slist: &SubstitutionList, texts: &[Vec<AugGlyph>]) -> io::Result<Vec<Vec<AugGlyph>>> {
  for text in texts {
    for g in text {
      match g {
        AugGlyph::Real(g) if g.char().is_ascii() => (),
        _ => panic!("hb-shape input must be ASCII letters or punctuation, got {:?}", g)
      }
    }
  }
  
  let fea_file = Builder::new().suffix(".fea").tempfile()?;
  let out_otf_file = Builder::new().suffix(".ttf").tempfile()?;
  let text_file = Builder::new().suffix(".txt").tempfile()?;
  
  let fea = format!("
{}
  
feature rlig {{
  {}
}} rlig;
", p::render_lc_class(test_font_glyph_names()?), p::render_fea_feature_body(slist));
  fs::write(&fea_file, fea)?;
  successful(Command::new("fonttools").args(["feaLib", "-o", p(&out_otf_file), p(&fea_file), TEST_FONT_PATH]).status()?, "fonttools")?;
  
  let lines: Vec<String> = texts.iter().map(|t| crate::glyphs::aug_encode(t)).collect();
  fs::write(&text_file, lines.join("\n") + "\n")?;
  let output = Command::new("hb-shape").args([p(&out_otf_file), &format!("--text-file={}", p(&text_file))]).output()?;
  successful(output.status, "hb-shape")?;
  
  from_utf8(&output.stdout).unwrap().lines().map(parse_shaping_output).collect()
}

fn test_font_glyph_names() -> io::Result<&'static HashSet<String>> {
  static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
  if let Some(names) = NAMES.get() {
    return Ok(names);
  }
  
  // fonttools ttx -q -t GlyphOrder -o - res/test-font.ttf
  let output = Command::new("fonttools").args(["ttx", "-q", "-t", "GlyphOrder", "-o", "-", TEST_FONT_PATH]).output()?;
  successful(output.status, "fonttools ttx")?;
  
  let names = GLYPH_ID_RE.captures_iter(from_utf8(&output.stdout).unwrap())
    .map(|c| c[1].to_owned())
    .collect();
  Ok(NAMES.get_or_init(|| names))
}

lazy_static! {
  static ref GLYPH_ID_RE: Regex = Regex::new(r#"<GlyphID id="\d+" name="([^"]+)"/>"#).unwrap();
}

fn parse_shaping_output(s: &str) -> io::Result<Vec<AugGlyph>> {
  // [y=0+400|u=1+1000]
  let s = s.trim().trim_start_matches('[').trim_end_matches(']');
  if s.is_empty() {
    return Ok(vec![]);
  }
  s.split('|').map(|item| {
    let name = item.split('=').next().unwrap();
    AugGlyph::from_name(name).ok_or(io::Error::new(io::ErrorKind::Other, format!("Unknown glyph name from hb-shape: {}", name)))
  }).collect()
}

#[cfg(test)]
mod stripping_tests {
  use super::*;
  #[test]
  fn test_stripping_1() {
    assert_eq!(crate::glyphs::aug_encode(&parse_shaping_output("[y=0+400|u=1+1000]").unwrap()), "yu".to_owned());
  }
  
  #[test]
  fn test_stripping_2() {
    use crate::glyphs::{AugGlyph::*, Glyph::*};
    assert_eq!(parse_shaping_output("[sh=0+1000|syn3=3+500|apos=4+1000|hyphen=6+850]\n").unwrap(), vec![Real(Sh), Synthetic(3), Real(Apos), Real(Hyphen)]);
  }
}

fn p(f: &NamedTempFile) -> &str { f.path().to_str().unwrap() }
fn p2(f: &Path) -> &str { f.to_str().unwrap() }

fn successful(e: ExitStatus, name: &str) -> io::Result<()> {
  match e.code().ok_or(io::Error::new(io::ErrorKind::Other, format!("No return code {}", name)))? {
    0 => Ok(()),
    code => Err(io::Error::new(io::ErrorKind::Other, format!("Nonzero return code {} {}", name, code)))
  }
}

#[cfg(test)]
mod tests {
  use crate::substitutions2::{SubstitutionList, Lookup};
  use super::*;
  
  fn r(g: crate::glyphs::Glyph) -> AugGlyph { AugGlyph::Real(g) }
  fn rr(g: &[crate::glyphs::Glyph]) -> Vec<AugGlyph> { g.iter().map(|g| r(*g)).collect() }

  #[test]
  fn test_1() {
    assert_eq!(apply_using_hbshape(&SubstitutionList { lookups: vec![Lookup { substitutions: vec![] }] }, &vec![]).unwrap(), vec![]);
  }
  
  #[test]
  fn test_2() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a by b;
          } l0;
        ").unwrap(),
        &rr(&[A])
      ).unwrap(),
      rr(&[B])
    );
  }
  
  #[test]
  fn test_3() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a' b by c;
            sub c b' by d;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[C, D])
    );
  }
  
  #[test]
  fn test_4() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a  b' by c;
            sub a' c  by d;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[A, C])
    );
  }
  
  #[test]
  fn test_5() {
    use crate::glyphs::Glyph::*;
    assert_eq!(
      apply_using_hbshape(
        &crate::fea_parsing::parse_fea_feature_body("
          lookup l0 {
            sub a' by c d;
            sub d' by e;
          } l0;
        ").unwrap(),
        &rr(&[A, B])
      ).unwrap(),
      rr(&[C, D, B])
    );
  }
}

