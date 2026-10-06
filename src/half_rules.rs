// Half-rules: the anteriors and posteriors of a first-pass HLSubstitutionList,
// flattened into one list of independent substitutions for the second pass.
//
// A half-rule is `pre[at]post → out`, optionally anchored to the start (^) or
// end ($) of a word, or anti-anchored (~) so it can't be at the start or end.
// Either `at` or `out` must be a single glyph, so the rule is an OpenType
// ligature (many → one) or multiple substitution (one → many), possibly with
// context. The inverse swaps `at` and `out` and keeps the
// context, so it is in the same class.
//
// Each glyph of context is a set of glyphs, any of which matches (a
// broadened rule, see SecondPass::broaden_sweep); usually it has just one.
// Sets of more than one are written in parentheses, e.g. (ae)t[o]→u, and
// become inline glyph classes in the font.

use crate::astarlike2::distance;
use crate::glyphs::{AugGlyph, aug_decode, aug_encode};
use crate::high_level_substitutions2::HLSubstitutionList;
use crate::substitutions2 as s2;

// A glyph of context: the glyphs that match there, sorted.
pub type GlyphSet = Vec<AugGlyph>;

// Context of single glyphs.
pub fn singletons(gs: &[AugGlyph]) -> Vec<GlyphSet> {
  gs.iter().map(|g| vec![*g]).collect()
}

fn context_matches(context: &[GlyphSet], word: &[AugGlyph]) -> bool {
  context.iter().zip(word).all(|(set, g)| set.contains(g))
}

fn encode_context(context: &[GlyphSet]) -> String {
  context.iter().map(|set| if set.len() == 1 { aug_encode(set) } else { format!("({})", aug_encode(set)) }).collect()
}

fn decode_context(text: &str) -> Result<Vec<GlyphSet>, String> {
  let mut res = vec![];
  let mut rest = text;
  while !rest.is_empty() {
    if let Some(after) = rest.strip_prefix('(') {
      let end = after.find(')').ok_or("No )")?;
      let mut set = aug_decode(&after[.. end]);
      set.sort();
      set.dedup();
      res.push(set);
      rest = &after[end + 1 ..];
    }
    else {
      let end = rest.find('(').unwrap_or(rest.len());
      res.extend(singletons(&aug_decode(&rest[.. end])));
      rest = &rest[end ..];
    }
  }
  Ok(res)
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HalfRule {
  pub pre: Vec<GlyphSet>,
  pub at: Vec<AugGlyph>,
  pub post: Vec<GlyphSet>,
  pub at_start: bool,
  pub at_end: bool,
  // The glyph before pre (after post) must be a word glyph.
  pub not_at_start: bool,
  pub not_at_end: bool,
  pub out: Vec<AugGlyph>
}

impl HalfRule {
  pub fn is_valid(&self) -> bool {
    !self.at.is_empty() && !self.out.is_empty() && (self.at.len() == 1 || self.out.len() == 1)
  }

  fn matches_at(&self, word: &[AugGlyph], pos: usize) -> bool {
    let (pre, at, post) = (self.pre.len(), self.at.len(), self.post.len());
    pos >= pre
      && pos + at + post <= word.len()
      && !(self.at_start && pos > pre && word[pos - pre - 1].is_word_glyph())
      && !(self.at_end && pos + at + post < word.len() && word[pos + at + post].is_word_glyph())
      && !(self.not_at_start && !(pos > pre && word[pos - pre - 1].is_word_glyph()))
      && !(self.not_at_end && !(pos + at + post < word.len() && word[pos + at + post].is_word_glyph()))
      && context_matches(&self.pre, &word[pos - pre .. pos])
      && word[pos .. pos + at] == self.at[..]
      && context_matches(&self.post, &word[pos + at .. pos + at + post])
  }

  // Scans left to right, replacing each match and resuming after the
  // replacement. Contexts see glyphs already replaced earlier in the scan, as
  // in an OpenType lookup. Returns whether anything was replaced.
  pub fn apply(&self, word: &mut Vec<AugGlyph>) -> bool {
    let mut pos = 0;
    let mut any_mod = false;
    while pos < word.len() {
      if self.matches_at(word, pos) {
        word.splice(pos .. pos + self.at.len(), self.out.iter().cloned());
        any_mod = true;
        pos += self.out.len();
      }
      else {
        pos += 1;
      }
    }
    any_mod
  }

  // `apply`, writing the result into `out` instead of editing `word` in
  // place. The output so far is the already-rewritten prefix that apply's
  // lookbehind would see.
  pub fn apply_into(&self, word: &[AugGlyph], out: &mut Vec<AugGlyph>) -> bool {
    let (pre, at, post) = (self.pre.len(), self.at.len(), self.post.len());
    out.clear();
    let mut i = 0;
    let mut any_mod = false;
    while i < word.len() {
      let pos = out.len();
      let matches =
           pos >= pre
        && i + at + post <= word.len()
        && !(self.at_start && pos > pre && out[pos - pre - 1].is_word_glyph())
        && !(self.at_end && i + at + post < word.len() && word[i + at + post].is_word_glyph())
        && !(self.not_at_start && !(pos > pre && out[pos - pre - 1].is_word_glyph()))
        && !(self.not_at_end && !(i + at + post < word.len() && word[i + at + post].is_word_glyph()))
        && context_matches(&self.pre, &out[pos - pre ..])
        && word[i .. i + at] == self.at[..]
        && context_matches(&self.post, &word[i + at .. i + at + post]);
      if matches {
        out.extend_from_slice(&self.out);
        i += at;
        any_mod = true;
      }
      else {
        out.push(word[i]);
        i += 1;
      }
    }
    any_mod
  }
  
  pub fn apply_copied(&self, word: &Vec<AugGlyph>) -> Option<Vec<AugGlyph>> {
    let mut word = word.clone();
    if self.apply(&mut word) { Some(word) } else { None }
  }

  pub fn inverse(&self) -> HalfRule {
    HalfRule {
      pre: self.pre.clone(),
      at: self.out.clone(),
      post: self.post.clone(),
      at_start: self.at_start,
      at_end: self.at_end,
      not_at_start: self.not_at_start,
      not_at_end: self.not_at_end,
      out: self.at.clone()
    }
  }

  // Same scale as the first pass's size cost.
  pub fn size_cost(&self) -> f64 {
    let size = self.pre.iter().map(|s| s.len()).sum::<usize>() + self.at.len() + self.post.iter().map(|s| s.len()).sum::<usize>()
      + (if self.at_start || self.not_at_start { 1 } else { 0 }) + (if self.at_end || self.not_at_end { 1 } else { 0 });
    (size as f64) * 0.001
  }

  pub fn encode(&self) -> String {
    format!("{}{}[{}]{}{}→{}",
      if self.at_start { "^" } else if self.not_at_start { "~" } else { "" },
      encode_context(&self.pre),
      aug_encode(&self.at),
      encode_context(&self.post),
      if self.at_end { "$" } else if self.not_at_end { "~" } else { "" },
      aug_encode(&self.out)
    )
  }

  pub fn decode(text: &str) -> Result<HalfRule, String> {
    if let [left, right] = text.split("→").collect::<Vec<_>>()[..] {
      let (at_start, not_at_start, left) = match (left.strip_prefix('^'), left.strip_prefix('~')) {
        (Some(rest), _) => (true, false, rest),
        (None, Some(rest)) => (false, true, rest),
        (None, None) => (false, false, left)
      };
      let (at_end, not_at_end, left) = match (left.strip_suffix('$'), left.strip_suffix('~')) {
        (Some(rest), _) => (true, false, rest),
        (None, Some(rest)) => (false, true, rest),
        (None, None) => (false, false, left)
      };
      let open = left.find('[').ok_or("No [")?;
      let close = left.rfind(']').ok_or("No ]")?;
      Ok(HalfRule {
        pre: decode_context(&left[.. open])?,
        at: aug_decode(&left[open + 1 .. close]),
        post: decode_context(&left[close + 1 ..])?,
        at_start,
        at_end,
        not_at_start,
        not_at_end,
        out: aug_decode(right)
      })
    }
    else {
      Err("Doesn't have 2 parts separated by →".to_owned())
    }
  }

  // The OpenType substitutions for this rule, which go in one lookup: ignore
  // rules that stop ^ and $ matching inside words, then the substitution.
  // An anti-anchor is an extra "any word glyph" of context.
  pub fn low_level(&self) -> Vec<s2::Substitution> {
    use s2::KeyElem;
    let glyphs = |gs: &[AugGlyph]| -> Vec<KeyElem> { gs.iter().map(|g| KeyElem::Glyph(*g)).collect() };
    let sets = |ss: &[GlyphSet]| -> Vec<KeyElem> {
      ss.iter().map(|s| if s.len() == 1 { KeyElem::Glyph(s[0]) } else { KeyElem::Set(s.clone()) }).collect()
    };
    let mut res = vec![];

    let mut pre = sets(&self.pre);
    if self.not_at_start {
      pre.insert(0, KeyElem::AnyLetter);
    }
    let mut post = sets(&self.post);
    if self.not_at_end {
      post.push(KeyElem::AnyLetter);
    }

    // Ignore rules mark only the first glyph of `at`; the rest is lookahead.
    let mut rest_and_post = glyphs(&self.at[1 ..]);
    rest_and_post.extend(post.iter().cloned());

    if self.at_start {
      let mut pre_key = vec![KeyElem::AnyLetter];
      pre_key.extend(pre.iter().cloned());
      res.push(s2::Substitution {
        pre_key,
        at_key: vec![self.at[0]],
        post_key: rest_and_post.clone(),
        sub_content: s2::SubContent::Ignore
      });
    }

    if self.at_end {
      let mut post_key = rest_and_post.clone();
      post_key.push(KeyElem::AnyLetter);
      res.push(s2::Substitution {
        pre_key: pre.clone(),
        at_key: vec![self.at[0]],
        post_key,
        sub_content: s2::SubContent::Ignore
      });
    }

    res.push(s2::Substitution {
      pre_key: pre,
      at_key: self.at.clone(),
      post_key: post,
      sub_content: s2::SubContent::Sub(self.out.clone())
    });

    res
  }
}

impl std::fmt::Debug for HalfRule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    f.write_str(&self.encode())
  }
}

// The anteriors in order, then the posteriors in reverse order, which is the
// order HLSubstitutionList::apply runs them in.
pub fn flatten(list: &HLSubstitutionList) -> Vec<HalfRule> {
  let mut res: Vec<HalfRule> = list.substitutions.iter().map(|s| HalfRule {
    pre: singletons(&s.anterior.pre_key),
    at: s.anterior.at_key.clone(),
    post: singletons(&s.anterior.post_key),
    at_start: s.anterior.at_start,
    at_end: s.anterior.at_end,
    not_at_start: s.anterior.not_at_start,
    not_at_end: s.anterior.not_at_end,
    out: vec![AugGlyph::Synthetic(s.mid)]
  }).collect();

  res.extend(list.substitutions.iter().rev().map(|s| HalfRule {
    pre: vec![],
    at: vec![AugGlyph::Synthetic(s.mid)],
    post: vec![],
    at_start: false,
    at_end: false,
    not_at_start: false,
    not_at_end: false,
    out: s.posterior.content.clone()
  }));

  res
}

pub fn apply_all(rules: &[HalfRule], word: &mut Vec<AugGlyph>) {
  for r in rules {
    r.apply(word);
  }
}

// apply_all using two scratch buffers; the result ends up in `a`.
pub fn apply_all_into(rules: &[HalfRule], word: &[AugGlyph], a: &mut Vec<AugGlyph>, b: &mut Vec<AugGlyph>) {
  a.clear();
  a.extend_from_slice(word);
  for r in rules {
    if r.apply_into(a, b) {
      std::mem::swap(a, b);
    }
  }
}

pub fn apply_all_copied(rules: &[HalfRule], word: &Vec<AugGlyph>) -> Vec<AugGlyph> {
  let mut word = word.clone();
  apply_all(rules, &mut word);
  word
}

// One lookup per rule.
pub fn low_level(rules: &[HalfRule]) -> s2::SubstitutionList {
  s2::SubstitutionList {
    lookups: rules.iter().map(|r| s2::Lookup { substitutions: r.low_level() }).collect()
  }
}

pub struct ScoringWord {
  pub spelling: Vec<AugGlyph>,
  pub pronunciation: Vec<AugGlyph>,
  pub frequency: f64
}

// Frequency-weighted distance of the output from the pronunciation, plus each
// rule's size cost. This is what the second pass minimizes.
pub fn global_score(rules: &[HalfRule], words: &[ScoringWord]) -> f64 {
  let distance_cost: f64 = words.iter().map(|w| {
    w.frequency * (distance(&apply_all_copied(rules, &w.spelling), &w.pronunciation) as f64)
  }).sum();
  let size_cost: f64 = rules.iter().map(|r| r.size_cost()).sum();
  distance_cost + size_cost
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::glyphs::AugGlyph::Real;

  fn aug(gs: &Vec<crate::glyphs::Glyph>) -> Vec<AugGlyph> {
    gs.iter().map(|g| Real(*g)).collect()
  }

  #[test]
  fn encode_decode_test() {
    for text in ["[th]→{0}", "^v[{4}]{12}→{66}", "[{11}]→t{9}", "[s]$→{7}"] {
      assert_eq!(HalfRule::decode(text).unwrap().encode(), text);
    }
  }

  #[test]
  fn apply_test() {
    // Matches can share context: both b's are replaced.
    let r = HalfRule::decode("a[b]a→{0}").unwrap();
    let mut w = aug_decode("ababa");
    r.apply(&mut w);
    assert_eq!(aug_encode(&w), "a{0}a{0}a");
    r.inverse().apply(&mut w);
    assert_eq!(aug_encode(&w), "ababa");
  }

  // The flattened rules must do exactly what the HL rule list does.
  #[test]
  fn flatten_matches_hl_apply() {
    let list = HLSubstitutionList::set_1();
    let rules = flatten(&list);
    assert_eq!(rules.len(), 2 * list.substitutions.len());
    assert!(rules.iter().all(|r| r.is_valid()));

    let dictionary = crate::dictionary::load_dictionary().unwrap();
    for w in &dictionary.words {
      let spelling = aug(&w.spelling);
      assert_eq!(
        aug_encode(&apply_all_copied(&rules, &spelling)),
        aug_encode(&list.apply_copied_always(&spelling)),
        "{}", aug_encode(&spelling)
      );
    }
  }

  // The low-level lookups must do exactly what the half-rules do.
  #[test]
  fn low_level_matches_apply() {
    let rules = flatten(&HLSubstitutionList::set_1());
    let slist = low_level(&rules);

    let dictionary = crate::dictionary::load_dictionary().unwrap();
    for w in &dictionary.words {
      let spelling = aug(&w.spelling);
      let mut by_low_level = spelling.clone();
      s2::apply_all(&mut by_low_level, &slist);
      assert_eq!(
        aug_encode(&by_low_level),
        aug_encode(&apply_all_copied(&rules, &spelling)),
        "{}", aug_encode(&spelling)
      );
    }
  }

  #[test]
  fn apply_into_matches_apply() {
    let rules = flatten(&HLSubstitutionList::set_1());
    let dictionary = crate::dictionary::load_dictionary().unwrap();
    let (mut a, mut b, mut out) = (vec![], vec![], vec![]);
    for w in dictionary.words.iter().take(3000) {
      let mut word = aug(&w.spelling);
      let spelling = word.clone();
      for r in &rules {
        let into = r.apply_into(&word, &mut out);
        let in_place = r.apply(&mut word);
        assert_eq!(into, in_place);
        assert_eq!(out, word, "{:?}", r);
      }
      apply_all_into(&rules, &spelling, &mut a, &mut b);
      assert_eq!(a, word);
    }
  }
  
  // Undoing a posterior and redoing it gives back the original.
  #[test]
  fn posterior_round_trip() {
    let rules = flatten(&HLSubstitutionList::set_1());
    let n = rules.len() / 2;
    let dictionary = crate::dictionary::load_dictionary().unwrap();
    for w in dictionary.words.iter().take(500) {
      let pronunciation = aug(&w.pronunciation);
      for r in &rules[n ..] {
        let mut undone = pronunciation.clone();
        r.inverse().apply(&mut undone);
        r.apply(&mut undone);
        assert_eq!(undone, pronunciation, "{:?}", r);
      }
    }
  }
}
