
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Glyph {
  A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
  Ch, Th, Sh, Jh, Ng, Er, Eh, Ah, Oi, Ow, Aw, Eu, Uh, Ee, Ei, Yu, Dh, Ae, Ih, Schwa, Apos, Hyphen
}

impl Glyph {
  pub fn name(&self) -> String {
    use Glyph::*;
    (match self {
      A => "a", B => "b", C => "c", D => "d", E => "e", F => "f", G => "g",
      H => "h", I => "i", J => "j", K => "k", L => "l", M => "m", N => "n",
      O => "o", P => "p", Q => "q", R => "r", S => "s", T => "t", U => "u",
      V => "v", W => "w", X => "x", Y => "y", Z => "z", Ch => "ch", Th => "th",
      Sh => "sh", Jh => "jh", Eh => "eh", Ah => "ah", Oi => "oi", Ow => "ow",
      Aw => "aw", Eu => "eu", Uh => "uh", Ee => "ee", Ei => "ei", Yu => "yu", Dh => "dh",
      Ng => "ng", Ae => "ae", Ih => "ih", Schwa => "schwa", Hyphen => "hyphen", Er => "er",
      Apos => "apos"
    }).to_owned()
  }
  
  pub fn from_name(text: &str) -> Option<Self> {
    use Glyph::*;
    Some(match text {
      "a" => A, "b" => B, "c" => C, "d" => D, "e" => E, "f" => F, "g" => G,
      "h" => H, "i" => I, "j" => J, "k" => K, "l" => L, "m" => M, "n" => N,
      "o" => O, "p" => P, "q" => Q, "r" => R, "s" => S, "t" => T, "u" => U,
      "v" => V, "w" => W, "x" => X, "y" => Y, "z" => Z, "ch" => Ch, "th" => Th,
      "sh" => Sh, "jh" => Jh, "eh" => Eh, "ah" => Ah, "oi" => Oi, "ow" => Ow,
      "aw" => Aw, "eu" => Eu, "uh" => Uh, "ee" => Ee, "ei" => Ei, "yu" => Yu, "dh" => Dh,
      "ng" => Ng, "ae" => Ae, "ih" => Ih, "schwa" => Schwa, "hyphen" => Hyphen, "er" => Er,
      "apos" => Apos, _ => return None
    })
  }
  
  pub fn char(&self) -> String {
    use Glyph::*;
    (match self {
      A => "a", B => "b", C => "c", D => "d", E => "e", F => "f", G => "g",
      H => "h", I => "i", J => "j", K => "k", L => "l", M => "m", N => "n",
      O => "o", P => "p", Q => "q", R => "r", S => "s", T => "t", U => "u",
      V => "v", W => "w", X => "x", Y => "y", Z => "z", Ch => "ʧ", Th => "θ",
      Sh => "ʃ", Jh => "ʒ", Eh => "ε", Ah => "ɑ", Oi => "ꭢ", Ow => "ʊ",
      Aw => "ɔ", Eu => "ɜ", Uh => "ʌ", Ee => "ɩ", Ei => "ϵ", Yu => "ū", Dh => "ϑ",
      Ng => "ŋ", Ae => "æ", Ih => "ɪ", Schwa => "ə", Hyphen => "-", Er => "ʳ",
      Apos => "'"
    }).to_owned()
  }
  
  pub fn from_char<'a>(ch: &'a str) -> Option<(Glyph, &'a str)> {
    use Glyph::*;
    for (g, text) in [
        (A, "a"), (B, "b"), (C, "c"), (D, "d"), (E, "e"), (F, "f"), (G, "g"),
        (H, "h"), (I, "i"), (J, "j"), (K, "k"), (L, "l"), (M, "m"), (N, "n"),
        (O, "o"), (P, "p"), (Q, "q"), (R, "r"), (S, "s"), (T, "t"), (U, "u"),
        (V, "v"), (W, "w"), (X, "x"), (Y, "y"), (Z, "z"), (Ch, "ʧ"), (Th, "θ"),
        (Sh, "ʃ"), (Jh, "ʒ"), (Eh, "ε"), (Ah, "ɑ"), (Oi, "ꭢ"), (Ow, "ʊ"),
        (Aw, "ɔ"), (Eu, "ɜ"), (Uh, "ʌ"), (Ee, "ɩ"), (Ei, "ϵ"), (Yu, "ū"), (Dh, "ϑ"),
        (Ng, "ŋ"), (Ae, "æ"), (Ih, "ɪ"), (Schwa, "ə"), (Hyphen, "-"), (Er, "ʳ"),
        (Apos, "'")]
    {
      match ch.strip_prefix(text) {
        None => (),
        Some(rest) => return Some((g, rest))
      }
    }
    None
  }
  
  pub fn is_vowel(&self) -> bool {
    use Glyph::*;
    (match self {
      A => true, B => false, C => false, D => false, E => true, F => false, G => false,
      H => false, I => true, J => false, K => false, L => false, M => false, N => false,
      O => true, P => false, Q => false, R => false, S => false, T => false, U => true,
      V => false, W => false, X => false, Y => false, Z => false, Ch => false, Th => false,
      Sh => false, Jh => false, Eh => true, Ah => true, Oi => true, Ow => true,
      Aw => true, Eu => true, Uh => true, Ee => true, Ei => true, Yu => true, Dh => false,
      Ng => false, Ae => true, Ih => true, Schwa => true, Hyphen => false, Er => false,
      Apos => false
    }).to_owned()
  }
  
  // Whether this glyph is part of a word, as opposed to a word boundary. This
  // decides what ^ and $ mean, both in the simulation and in the font's @lc
  // class (see fea_parsing::render_lc_class), so the two must agree.
  // Apostrophes are inside words ("don't"); hyphens separate them.
  pub fn is_word_glyph(&self) -> bool {
    use Glyph::*;
    (match self {
      A|B|C|D|E|F|G|H|I|J|K|L|M|N|O|P|Q|R|S|T|U|V|W|X|Y|Z|
      Ch|Th|Sh|Jh|Ng|Er|Eh|Ah|Oi|Ow|Aw|Eu|Uh|Ee|Ei|Yu|Dh|Ae|Ih|Schwa|Apos => true,
      Hyphen => false
    }).to_owned()
  }
  
  pub fn all() -> Vec<Glyph> {
    use Glyph::*;
    vec![A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
      Ch, Th, Sh, Jh, Ng, Er, Eh, Ah, Oi, Ow, Aw, Eu, Uh, Ee, Ei, Yu, Dh, Ae, Ih, Schwa, Apos, Hyphen]
  }
}

pub const MAX_SYN: usize = 1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AugGlyph {
  Real(Glyph),
  Synthetic(u32)
}

impl AugGlyph {
  pub fn name(&self) -> String {
    match self {
      Self::Real(g) => g.name(),
      Self::Synthetic(n) => format!("syn{}", n)
    }
  }
  
  pub fn from_name(text: &str) -> Option<Self> {
    if text.starts_with("syn") {
      Some(Self::Synthetic(text[3..].parse().ok()?))
    }
    else {
      Some(Self::Real(Glyph::from_name(text)?))
    }
  }
  
  pub fn is_word_glyph(&self) -> bool {
    match self {
      AugGlyph::Real(g) => g.is_word_glyph(),
      AugGlyph::Synthetic(_) => true
    }
  }
}

pub fn asnames(gs: &Vec<Glyph>) -> String {
  let mut res = "".to_owned();
  for g in gs {
    res.extend(g.name().chars())
  }
  res
}

pub fn encode(gs: &Vec<Glyph>) -> String {
  let mut res = "".to_owned();
  
  for g in gs {
    res.extend(g.char().chars());
  }
  
  res
}

pub fn decode(text: &str) -> Vec<Glyph> {
  let mut res = vec![];
  let mut working = text;
  
  while !working.is_empty() {
    match Glyph::from_char(working) {
      Some((g, rest)) => {
        res.push(g);
        working = rest;
      },
      None => {
        working = &working[1..];
      }
    }
  }
  
  res
}

pub fn aug_encode(gs: &Vec<AugGlyph>) -> String {
  let mut res = "".to_owned();
  
  for g in gs {
    match g {
      AugGlyph::Real(g) => res.extend(g.char().chars()),
      AugGlyph::Synthetic(n) => res.extend(format!("{{{}}}", n).chars())
    };
  }
  
  res
}

pub fn aug_decode(text: &str) -> Vec<AugGlyph> {
  let mut res = vec![];
  let mut working = text;
  
  while !working.is_empty() {
    if working.chars().next().unwrap() == '{' {
      working = &working[1..];
      let mut number = "".to_owned();
      while !working.is_empty() {
        let ch = working.chars().next().unwrap();
        if ch == '}' {
          working = &working[1..];
          break;
        }
        number.extend(&[ch]);
        working = &working[1..];
      }
      match number.parse() {
        Ok(k) => res.push(AugGlyph::Synthetic(k)),
        Err(_) => ()
      }
    }
    else {
      match Glyph::from_char(working) {
        Some((g, rest)) => {
          res.push(AugGlyph::Real(g));
          working = rest;
        },
        None => {
          working = &working[1..];
        }
      }
    }
  }
  
  res
}

pub fn augment(v: &Vec<Glyph>) -> Vec<AugGlyph> {
  v.iter().map(|g| AugGlyph::Real(*g)).collect()
}

pub fn strip_aug(v: &Vec<AugGlyph>) -> Vec<Glyph> {
  v.iter().filter_map(|g| match g {
    AugGlyph::Real(g) => Some(*g),
    _ => None
  }).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use Glyph::*;
  use AugGlyph::*;

  #[test]
  fn test_encode_1() {
    assert_eq!(encode(&vec![]), "".to_owned());
  }
  
  #[test]
  fn test_encode_2() {
    assert_eq!(encode(&vec![A, B, C]), "abc".to_owned());
  }
  
  #[test]
  fn test_decode_1() {
    assert_eq!(decode(""), vec![]);
  }
  
  #[test]
  fn test_decode_2() {
    assert_eq!(decode("abc"), vec![A, B, C]);
  }
  
  #[test]
  fn test_aug_decode() {
    assert_eq!(aug_decode("a"), vec![Real(A)]);
    assert_eq!(aug_decode("a{0}"), vec![Real(A), Synthetic(0)]);
    assert_eq!(aug_decode("a{0}b"), vec![Real(A), Synthetic(0), Real(B)]);
    assert_eq!(aug_decode("{0}b"), vec![Synthetic(0), Real(B)]);
    assert_eq!(aug_decode("{110}b"), vec![Synthetic(110), Real(B)]);
  }
  
  #[test]
  fn test_aug_encode() {
    assert_eq!(&aug_encode(&aug_decode("a")), "a");
    assert_eq!(&aug_encode(&aug_decode("a{0}")), "a{0}");
    assert_eq!(&aug_encode(&aug_decode("{0}a")), "{0}a");
    assert_eq!(&aug_encode(&aug_decode("a{3}b")), "a{3}b");
    assert_eq!(&aug_encode(&aug_decode("---{7}")), "---{7}");
  }
}

