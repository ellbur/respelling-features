
use itertools::Itertools;
use nom::bytes::complete::tag;
use nom::IResult;
use nom::branch::alt;
use nom::Parser;
use nom::multi::{many0, many1};

// ˌekspləˈneɪʃən
// ˈmeʒəR
// ˌnəʊ ˈmætəR ˌhaʊ
// ˈempti
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Copy, Clone)]
pub enum Vowel {
  Ve, Və, Veɪ, Vəʊ, Væ, Vaʊ, VəR, Vi,
  Viː, Vɪ, Vʌ, Vaɪ, Vɒ, Vuː, VɑːR, VⱭː, VɔːR, VɜːR, Vɔː, Vʊ, Vɔɪ, Vu,
  VI, VƏ, Vɑː, Vɜː, Vɪə, VɪəR, Veə, Vʊə, VeəR, VʊəR,
}

impl Vowel {
  pub fn to_ipa(&self) -> String {
    use Vowel::*;
    (match self {
      Ve => "e",
      Və => "ə",
      Veɪ => "eɪ",
      Vəʊ => "əʊ",
      Væ => "æ",
      Vaʊ => "aʊ",
      VəR => "əR",
      Vi => "i",
      Viː => "iː",
      Vɪ => "ɪ",
      Vʌ => "ʌ",
      Vaɪ => "aɪ",
      Vɒ => "ɒ",
      Vuː => "uː",
      VɑːR => "ɑːR",
      VⱭː => "Ɑː",
      VɔːR => "ɔːR",
      VɜːR => "ɜːR",
      Vɔː => "ɔː",
      Vʊ => "ʊ",
      Vɔɪ => "ɔɪ",
      Vu => "u",
      VI => "I",
      VƏ => "Ə",
      Vɑː => "ɑː",
      Vɜː => "ɜː",
      Vɪə => "ɪə",
      VɪəR => "ɪəR",
      Veə => "eə",
      Vʊə => "ʊə",
      VeəR => "eəR",
      VʊəR => "ʊəR",
    }).to_string()
  }
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Copy, Clone)]
pub enum Consonant {
  Ck, Cs, Cp, Cl, Cn, Cʃ, Cm, Cʒ, Ct, Ch, Cz, Cw, Cð, Cj, Cb, Cθ, Cv, Cd, Cf, Cr, Cʍ, Ctʃ, Cɡ, Cŋ, Cdʒ,
}

impl Consonant {
  pub fn to_ipa(&self) -> String {
    use Consonant::*;
    (match self {
      Ck => "k",
      Cs => "s",
      Cp => "p",
      Cl => "l",
      Cn => "n",
      Cʃ => "ʃ",
      Cm => "m",
      Cʒ => "ʒ",
      Ct => "t",
      Ch => "h",
      Cz => "z",
      Cw => "w",
      Cð => "ð",
      Cj => "j",
      Cb => "b",
      Cθ => "θ",
      Cv => "v",
      Cd => "d",
      Cf => "f",
      Cr => "r",
      Cʍ => "ʍ",
      Ctʃ => "tʃ",
      Cɡ => "ɡ",
      Cŋ => "ŋ",
      Cdʒ => "dʒ",
    }).to_string()
  }
}

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Copy, Clone)]
pub enum Stress {
  Primary,
  Secondary,
  Unstressed
}

impl Stress {
  pub fn to_ipa(&self) -> String {
    use Stress::*;
    (match self {
      Primary => "ˈ",
      Secondary => "ˌ",
      Unstressed => ""
    }).to_string()
  }
}

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Clone)]
pub struct Syllable {
  pub initial_consonants: Vec<Consonant>,
  pub vowel: Vowel,
  pub final_consonants: Vec<Consonant>,
  pub stress: Stress
}

impl Syllable {
  pub fn to_ipa(&self) -> String {
    format!("{}{}{}{}",
      self.stress.to_ipa(),
      self.initial_consonants.iter().map(|c| c.to_ipa()).join(""),
      self.vowel.to_ipa(),
      self.final_consonants.iter().map(|v| v.to_ipa()).join("")
    )
  }

  pub fn parts_to_ipa(&self) -> (String, String, String, String) {
    (
      self.stress.to_ipa(),
      self.initial_consonants.iter().map(|c| c.to_ipa()).join(""),
      self.vowel.to_ipa(),
      self.final_consonants.iter().map(|v| v.to_ipa()).join("")
    )
  }
}

pub fn parse_consonant(i: &str) -> IResult<&str, Consonant> {
  use Consonant::*;
  alt((
    alt((
      tag("tʃ").map(|_| Ctʃ),
      tag("k").map(|_| Ck),
      tag("s").map(|_| Cs),
      tag("p").map(|_| Cp),
      tag("l").map(|_| Cl),
      tag("n").map(|_| Cn),
      tag("ʃ").map(|_| Cʃ),
      tag("m").map(|_| Cm),
      tag("ʒ").map(|_| Cʒ),
      tag("t").map(|_| Ct),
      tag("h").map(|_| Ch),
      tag("dʒ").map(|_| Cdʒ),
    )),
    alt((
      tag("z").map(|_| Cz),
      tag("w").map(|_| Cw),
      tag("ð").map(|_| Cð),
      tag("j").map(|_| Cj),
      tag("b").map(|_| Cb),
      tag("θ").map(|_| Cθ),
      tag("v").map(|_| Cv),
      tag("d").map(|_| Cd),
      tag("f").map(|_| Cf),
      tag("r").map(|_| Cr),
      tag("ʍ").map(|_| Cʍ),
      tag("ɡ").map(|_| Cɡ),
      tag("g").map(|_| Cɡ),
      tag("ŋ").map(|_| Cŋ),
    ))
  ))(i)
}

pub fn parse_vowel(i: &str) -> IResult<&str, Vowel> {
  use Vowel::*;
  // Ve, Və, Veɪ, Vəʊ, Væ, Vaʊ, RəR, Ri,
  alt((
    alt((
      tag("əR").map(|_| VəR),
      tag("ɔːR").map(|_| VɔːR),
      tag("ɜːR").map(|_| VɜːR),
      tag("ɑːR").map(|_| VɑːR),
      tag("ɪəR").map(|_| VɪəR),
      tag("eəR").map(|_| VeəR),
      tag("ʊəR").map(|_| VʊəR),
    )),
    alt((
      tag("eɪ").map(|_| Veɪ),
      tag("əʊ").map(|_| Vəʊ),
      tag("aʊ").map(|_| Vaʊ),
      tag("aɪ").map(|_| Vaɪ),
      tag("uː").map(|_| Vuː),
      tag("iː").map(|_| Viː),
      tag("ɔː").map(|_| Vɔː),
      tag("Ɑː").map(|_| VⱭː),
      tag("ɔɪ").map(|_| Vɔɪ),
      tag("ɑː").map(|_| Vɑː),
      tag("ɜː").map(|_| Vɜː),
      tag("ɪə").map(|_| Vɪə),
      tag("eə").map(|_| Veə),
      tag("ʊə").map(|_| Vʊə),
    )),
    alt((
      tag("i").map(|_| Vi),
      tag("e").map(|_| Ve),
      tag("ə").map(|_| Və),
      tag("ɪ").map(|_| Vɪ),
      tag("ʌ").map(|_| Vʌ),
      tag("æ").map(|_| Væ),
      tag("ɒ").map(|_| Vɒ),
      tag("ʊ").map(|_| Vʊ),
      tag("u").map(|_| Vu),
      tag("I").map(|_| VI),
      tag("Ə").map(|_| VƏ),
    )),
  ))(i)
}

pub fn parse_stress(i: &str) -> IResult<&str, Stress> {
  use Stress::*;
  // ˌnəʊ ˈmætəR ˌhaʊ
  alt((
    tag("ˈ").map(|_| Primary),
    tag("ˌ").map(|_| Secondary),
    tag("").map(|_| Unstressed)
  ))(i)
}

pub fn parse_double_split(i: &str) -> IResult<&str, &str> {
  alt((
    tag("+"),
    tag("")
  ))(i)
}

pub fn parse_syllable(i: &str) -> IResult<&str, Syllable> {
  let (i, _) = parse_double_split(i)?;
  let (i, stress) = parse_stress(i)?;
  let (i, initial_consonants) = many0(parse_consonant)(i)?;
  let (i, vowel) = parse_vowel(i)?;
  let (i, final_consonants) = many0(parse_consonant)(i)?;
  Ok((i, Syllable {
    initial_consonants,
    vowel,
    final_consonants,
    stress
  }))
}

pub fn distribute_consonants(syllables: &mut Vec<Syllable>) {
  for i in 0 .. syllables.len()-1 {
    if syllables[i+1].stress == Stress::Unstressed {
      let s1_count = syllables[i].final_consonants.len();
      let s2_count = syllables[i+1].initial_consonants.len();
      let total_count = s1_count + s2_count;
      let max_acceptable = {
        if total_count == 1 { 0 }
        else if total_count == 0 { 0 }
        else { 1 }
      };
      if s1_count > max_acceptable {
        let num_to_move = s1_count - max_acceptable;
        let chunk_to_move = syllables[i].final_consonants.split_off(s1_count - num_to_move);
        syllables[i+1].initial_consonants.splice(0 .. 0, chunk_to_move);
      }
    }
  }
}

pub fn parse_ipa(i: &str) -> IResult<&str, Vec<Syllable>> {
  let (i, mut syllables) = many1(parse_syllable)(i)?;
  distribute_consonants(&mut syllables);
  Ok((i, syllables))
}

pub fn vowel_to_pronunciation_glyphs(v: &Vowel) -> Vec<crate::glyphs::Glyph> {
  use crate::glyphs::Glyph::*;
  use Vowel::*;

  match v {
    Ve => vec![Eh], Və => vec![Schwa], Veɪ => vec![Ei], Vəʊ => vec![O], Væ => vec![Ae], Vaʊ => vec![Ow],
    VəR => vec![Er], Vi => vec![Ee],
    Viː => vec![Ee], Vɪ => vec![Ih], Vʌ => vec![Uh], Vaɪ => vec![I],
    Vɒ => vec![Ah], Vuː => vec![U], VɑːR => vec![Ah, R], VⱭː => vec![Ae],
    VɔːR => vec![Aw, R], VɜːR => vec![Er], Vɔː => vec![Aw], Vʊ => vec![Eu],
    Vɔɪ => vec![Oi], Vu => vec![U],
    VI => vec![Ih], VƏ => vec![Schwa], Vɑː => vec![Ah], Vɜː => vec![Uh],
    // ReadLex's British centring vowels, as an American accent says them
    // (here, sure). Vɪə on its own is "ee", or "ee" + schwa where it's
    // really two vowels (see ipa_to_pronunciation_glyphs).
    Vɪə => vec![Ee], VɪəR => vec![Ee, R], Veə => vec![Ei], Vʊə => vec![Eu],
    VeəR => vec![Ei, R], VʊəR => vec![Eu, R],
  }
}

pub fn consonant_to_pronunciation_glyphs(c: &Consonant) -> Vec<crate::glyphs::Glyph> {
  use crate::glyphs::Glyph;
  use crate::glyphs::Glyph::*;
  use Consonant::*;

  match c {
    Ck => vec![K], Cs => vec![S], Cp => vec![P], Cl => vec![L], Cn => vec![N],
    Cʃ => vec![Sh], Cm => vec![M],
    Cʒ => vec![Jh], Ct => vec![T], Consonant::Ch => vec![H], Cz => vec![Z], Cw => vec![W],
    Cð => vec![Dh], Cj => vec![Y], Cb => vec![B],
    Cθ => vec![Th], Cv => vec![V], Cd => vec![D], Cf => vec![F], Cr => vec![R], Cʍ => vec![W],
    Ctʃ => vec![Glyph::Ch], Cɡ => vec![G], Cŋ => vec![Ng], Cdʒ => vec![J]
  }
}

// A syllable's vowel glyphs. ReadLex writes ɪə both for the British "near"
// vowel (real, series) and for "ee" + schwa (media, area, idea, ideas), so an
// unstressed ɪə, or a stressed one in the last syllable that isn't followed
// by l or r, is "ee" + schwa. A word of one syllable counts as stressed.
fn syllable_vowel_glyphs(syll: &Syllable, last: bool, only: bool) -> Vec<crate::glyphs::Glyph> {
  use crate::glyphs::Glyph::*;
  let liquid_follows = matches!(syll.final_consonants.first(), Some(Consonant::Cl) | Some(Consonant::Cr));
  let stressed = only || syll.stress != Stress::Unstressed;
  if syll.vowel == Vowel::Vɪə && (!stressed || (last && !liquid_follows)) {
    return vec![Ee, Schwa];
  }
  vowel_to_pronunciation_glyphs(&syll.vowel)
}

pub fn ipa_to_pronunciation_glyphs(i: &str) -> Result<Vec<crate::glyphs::Glyph>, String> {
  let ipa = parse_ipa(i).map_err(|p| p.to_string())?.1;
  let n = ipa.len();

  Ok(ipa.into_iter().enumerate().flat_map(|(k, syll)| {
    let vowel = syllable_vowel_glyphs(&syll, k + 1 == n, n == 1);
    syll.initial_consonants.into_iter().flat_map(|c| consonant_to_pronunciation_glyphs(&c).into_iter())
      .chain(vowel.into_iter())
      .chain(syll.final_consonants.into_iter().flat_map(|c| consonant_to_pronunciation_glyphs(&c).into_iter()))
  }).collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Copy, Clone)]
  enum Choice {
    Ab,
    A,
    B,
  }

  fn parser_1(s: &str) -> IResult<&str, &str> {
    tag("ab")(s)
  }

  fn parser_2(s: &str) -> IResult<&str, Choice> {
    use Choice::*;
    alt((
      tag("ab").map(|_| Ab),
      tag("a").map(|_| A),
      tag("b").map(|_| B)
    ))(s)
  }

  #[test]
  fn test_parsing_1() {
    assert_eq!(parser_1("abc"), Ok(("c", "ab")));
  }

  #[test]
  fn test_parsing_2() {
    assert_eq!(parser_2("abc"), Ok(("c", Choice::Ab)));
  }

  #[test]
  fn test_ipa_1() {
    use Consonant::*;
    use Vowel::*;
    use Stress::*;

    assert_eq!(parse_ipa("ˈempti").unwrap().1, vec![
      Syllable {
        initial_consonants: vec![],
        vowel: Ve,
        final_consonants: vec![Cm, Cp],
        stress: Primary
      },
      Syllable {
        initial_consonants: vec![Ct],
        vowel: Vi,
        final_consonants: vec![],
        stress: Unstressed
      }
    ])
  }

  #[test]
  fn test_ipa_2() {
    use Consonant::*;
    use Vowel::*;
    use Stress::*;

    assert_eq!(parse_ipa("ˌekspləˈneɪʃən").unwrap().1, vec![
      Syllable {
        initial_consonants: vec![],
        vowel: Ve,
        final_consonants: vec![Ck],
        stress: Secondary
      },
      Syllable {
        initial_consonants: vec![Cs, Cp, Cl],
        vowel: Və,
        final_consonants: vec![],
        stress: Unstressed
      },
      Syllable {
        initial_consonants: vec![Cn],
        vowel: Veɪ,
        final_consonants: vec![],
        stress: Primary
      },
      Syllable {
        initial_consonants: vec![Cʃ],
        vowel: Və,
        final_consonants: vec![Cn],
        stress: Unstressed
      }
    ])
  }

  #[test]
  fn test_ipa_3() {
    use Consonant::*;
    use Vowel::*;
    use Stress::*;

    assert_eq!(parse_ipa("gʌv").unwrap().1, vec![
      Syllable {
        initial_consonants: vec![Cɡ],
        vowel: Vʌ,
        final_consonants: vec![Cv],
        stress: Secondary
      },
    ])
  }
}

