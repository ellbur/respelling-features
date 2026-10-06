#![allow(uncommon_codepoints)]

// Builds res/pronunciations-phonemic-ipa.txt: the top American words with
// their unstressed vowels treated consistently.
//
// Dictionaries mix phonetic and phonemic transcription, especially for
// unstressed vowels: ReadLex writes a reduced ɪ in "explain", which no one
// would think of as a short i. decided-pronunciations-ipa.txt corrects many
// words by hand, but not consistently, and most words aren't in it. So
// wherever ReadLex has an unstressed ɪ (in a word of more than one
// syllable), the vowel is decided here by one policy, derived from what the
// hand corrections usually do:
//
//   - ɪ before ŋ stays ɪ (-ing, increasingly, meaningful).
//   - Endings -ing(s), -ic(s), -ive(s), -ish, -ship(s), -is, -ist(s) and
//     -it(s) keep ɪ, as do closed first syllables spelled in-/im-.
//   - A first syllable spelled ex-, or a closed one spelled en-/em-, is e
//     (explain, exact, exist, example, entire, employ).
//   - The middle syllables of any-/every- compounds are iː (anybody).
//   - Everything else is ə (remember, decide, because, president, college,
//     market, office, private, stupid).
//
// Every other part of a pronunciation comes from the hand corrections where
// there is one, otherwise from ReadLex. A few words also get a chosen stress
// or American form (OVERRIDES).
//
// ReadLex is British in places, so then the pronunciations are made American
// (Central New Jersey, as far as the vowels go), checking each word against
// CMUdict (american_edits):
//
//   - ɒ is "ah" (hot, on, and orange, sorry, tomorrow before r), except
//     where CMUdict has "aw": the "cloth" words dog, long, off, lost, gone.
//   - The "bath" vowel (ask, last) is æ (see ipa.rs), except where
//     CMUdict has "ah" (drama).
//   - əʊ where CMUdict has "ah" (process, progress), and stressed aɪ where
//     it has "ee" (either, neither).
//   - The -ary/-ory syllable before the final -ry keeps its full vowel
//     where CMUdict has one (military, necessary, category).
//   - The aɪ of -ization is ə (organization).
//   - No y after t, d, s, z, n, l or θ starting a stressed syllable
//     (during, assume, pursue).
//
// Writes a report of the changes to working/phonemic-changes.txt.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use nom::multi::many0;

use feature_refining::glyphs::{Glyph, encode};
use feature_refining::ipa::{consonant_to_pronunciation_glyphs, ipa_to_pronunciation_glyphs, parse_consonant, parse_double_split, parse_ipa, parse_stress, parse_vowel, vowel_to_pronunciation_glyphs, Stress, Syllable, Vowel};
use feature_refining::readlex::{load_pronunciation_tweaks, read_readlex_top_american5000};

const OVERRIDES: &[(&str, &str)] = &[
  // REsearch, not reSEARCH.
  ("research", "ˈriːsɜːRtʃ"),
  ("researchers", "ˈriːsɜːRtʃəRz"),
  // American forms ReadLex doesn't have.
  ("privacy", "ˈpraɪvəsiː"),
  ("figure", "ˈfɪɡjəR"),
  ("figures", "ˈfɪɡjəRz"),
  ("figured", "ˈfɪɡjəRd"),
  ("mobile", "ˈməʊbəl"),
  ("yeah", "jæ"),
  ("data", "ˈdeɪtə"),
  // REE-uh-lize (ReadLex's ɪə would be "ee" here, as in "really").
  ("realize", "ˈriːəlaɪz"),
  ("realized", "ˈriːəlaɪzd"),
  // Unstressed -ior/-ear endings: "yer" or "ee-er", not the "near" vowel.
  ("junior", "ˈdʒuːnjəR"),
  ("senior", "ˈsiːnjəR"),
  ("earlier", "ˈɜːRliːəR"),
  ("nuclear", "ˈnuːkliːəR"),
  ("interior", "ɪnˈtɪəriːəR"),
  ("superior", "suːˈpɪəriːəR"),
  // Fixes from a review of the 5000 words: British forms, other words or
  // senses ReadLex chose, and conversion glitches.
  ("israel", "ˈɪzriːəl"),
  ("ai", "ˌeɪˈaɪ"),
  ("ceo", "ˌsiːiːˈəʊ"),
  ("fbi", "ˌefbiːˈaɪ"),
  ("cia", "ˌsiːaɪˈeɪ"),
  ("lt", "luːˈtenənt"),
  ("lieutenant", "luːˈtenənt"),
  ("mayor", "ˈmeɪəR"),
  ("tears", "tɪəRz"),
  ("row", "rəʊ"),
  ("sake", "seɪk"),
  ("mate", "meɪt"),
  ("grave", "ɡreɪv"),
  ("piano", "piːˈænəʊ"),
  ("min", "mɪn"),
  ("morgan", "ˈmɔːRɡən"),
  ("surveillance", "səRˈveɪləns"),
  ("referring", "rəˈfɜːRɪŋ"),
  ("refugees", "ˌrefjuːˈdʒiːz"),
  ("arthur", "ˈɑːRθəR"),
  ("francis", "ˈfrænsɪs"),
  ("garage", "ɡəˈrɑːʒ"),
  ("lawrence", "ˈlɔːrəns"),
  ("kong", "ˈkɔːŋ"),
  ("washington", "ˈwɒʃɪŋtən"),
];

fn ends_with_any(word: &str, endings: &[&str]) -> bool {
  endings.iter().any(|e| word.ends_with(e))
}

// The vowel for an unstressed ɪ in syllable i of n (n > 1), and the name of
// the rule that decided it.
fn decide(word: &str, i: usize, n: usize, syllable: &Syllable) -> (Vowel, &'static str) {
  let closed = !syllable.final_consonants.is_empty();
  // -ing- inside a word (increasingly, meaningful).
  if syllable.final_consonants.first() == Some(&feature_refining::ipa::Consonant::Cŋ) {
    return (Vowel::Vɪ, "before ŋ -> ɪ");
  }
  if i == 0 {
    if word.starts_with("ex") {
      return (Vowel::Ve, "first: ex- -> e");
    }
    if (word.starts_with("en") || word.starts_with("em")) && closed {
      return (Vowel::Ve, "first: en-/em- -> e");
    }
    if (word.starts_with("in") || word.starts_with("im")) && closed {
      return (Vowel::Vɪ, "first: in-/im- -> ɪ");
    }
    return (Vowel::Və, "first: other -> ə");
  }
  if i == n - 1 {
    // An open final syllable in a word ending in a vowel letter is the happY
    // vowel ("jesse"), not a reduced one.
    if !closed && ends_with_any(word, &["e", "y", "ie", "i"]) {
      return (Vowel::Viː, "last: open, spelled with a final vowel -> iː");
    }
    let keep = [
      "ing", "ings", "ic", "ics", "ive", "ives", "ish", "ship", "ships",
      "is", "ist", "ists", "it", "its"
    ];
    if ends_with_any(word, &keep) {
      return (Vowel::Vɪ, "last: -ing/-ic/-ive/-ish/-ship/-is/-ist/-it -> ɪ");
    }
    return (Vowel::Və, "last: other -> ə");
  }
  if word.starts_with("any") || word.starts_with("every") {
    return (Vowel::Viː, "middle: any-/every- -> iː");
  }
  (Vowel::Və, "middle: -> ə")
}

// The byte ranges of each syllable's vowel in an IPA string, parsed the same
// way as ipa::parse_ipa (whose consonant redistribution doesn't move vowels).
fn vowel_spans(ipa: &str) -> Vec<(usize, usize)> {
  let mut spans = vec![];
  let mut rest = ipa;
  while !rest.is_empty() {
    let (r, _) = parse_double_split(rest).unwrap();
    let (r, _) = parse_stress(r).unwrap();
    let (r, _) = many0(parse_consonant)(r).unwrap();
    let start = ipa.len() - r.len();
    let (r, _) = parse_vowel(r).unwrap();
    let end = ipa.len() - r.len();
    let (r, _) = many0(parse_consonant)(r).unwrap();
    spans.push((start, end));
    rest = r;
  }
  spans
}

// `ipa` with the vowels of the given syllables replaced, leaving everything
// else in the text as it was.
fn replace_vowels(ipa: &str, replacements: &[(usize, Vowel)]) -> String {
  let spans = vowel_spans(ipa);
  let mut out = ipa.to_owned();
  let mut replacements = replacements.to_vec();
  replacements.sort_by_key(|r| std::cmp::Reverse(r.0));
  for (i, vowel) in replacements {
    let (start, end) = spans[i];
    out.replace_range(start .. end, &vowel.to_ipa());
  }
  out
}

// Words CMUdict gives "aw" that keep "ah" to match their relatives: online
// (on), wanted (want, wants, wanting).
// Also some where "ah" is the usual American form (Wallace, Oscar, Moscow,
// blog).
const CLOTH_EXCEPTIONS: &[&str] = &["online", "wanted", "wallace", "oscar", "moscow", "blog"];

// Pairs (i, j) of positions of `a` and `b` that a minimal Levenshtein
// alignment matches or substitutes.
fn aligned(a: &[Glyph], b: &[Glyph]) -> Vec<(usize, usize)> {
  let (n, m) = (a.len(), b.len());
  let mut d = vec![vec![0u32; m + 1]; n + 1];
  for i in 0 ..= n { d[i][0] = i as u32; }
  for j in 0 ..= m { d[0][j] = j as u32; }
  for i in 1 ..= n {
    for j in 1 ..= m {
      let sub = d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 };
      d[i][j] = sub.min(d[i - 1][j] + 1).min(d[i][j - 1] + 1);
    }
  }
  let mut res = vec![];
  let (mut i, mut j) = (n, m);
  while i > 0 && j > 0 {
    if d[i][j] == d[i - 1][j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 } {
      res.push((i - 1, j - 1));
      i -= 1;
      j -= 1;
    }
    else if d[i][j] == d[i - 1][j] + 1 { i -= 1; }
    else { j -= 1; }
  }
  res
}

// The vowel changes (syllable, new vowel, rule) that make `ipa` American,
// judged against CMUdict's pronunciation `cmu` where one is given.
fn american_edits(word: &str, ipa: &str, cmu: Option<&Vec<Glyph>>) -> Vec<(usize, Vowel, &'static str)> {
  let Ok((_, syllables)) = parse_ipa(ipa) else { return vec![] };
  let n = syllables.len();
  let mut edits = vec![];

  // -ization: aɪ → ə.
  if word.contains("ization") {
    for (i, s) in syllables.iter().enumerate() {
      if s.vowel == Vowel::Vaɪ && s.stress == Stress::Unstressed {
        edits.push((i, Vowel::Və, "-ization: aɪ -> ə"));
      }
    }
  }

  let Some(cmu) = cmu else { return edits };
  // Our glyphs, with the position of each syllable's vowel.
  let mut ours: Vec<Glyph> = vec![];
  let mut vowel_at = vec![];
  for s in &syllables {
    ours.extend(s.initial_consonants.iter().flat_map(consonant_to_pronunciation_glyphs));
    vowel_at.push(ours.len());
    ours.extend(vowel_to_pronunciation_glyphs(&s.vowel));
    ours.extend(s.final_consonants.iter().flat_map(consonant_to_pronunciation_glyphs));
  }
  let pairs: HashMap<usize, usize> = aligned(&ours, cmu).into_iter().collect();
  let ends_ary = ["ary", "aries", "ory", "ories"].iter().any(|e| word.ends_with(e));

  for (i, s) in syllables.iter().enumerate() {
    let Some(&j) = pairs.get(&vowel_at[i]) else { continue };
    let theirs = cmu[j];
    let next = ours.get(vowel_at[i] + 1).cloned();
    let edit = match (s.vowel, theirs) {
      (Vowel::Vɒ, Glyph::Aw) if next != Some(Glyph::R) && !CLOTH_EXCEPTIONS.contains(&word) => Some((Vowel::Vɔː, "cloth: ɒ -> ɔː (CMUdict)")),
      (Vowel::VⱭː, Glyph::Ah) => Some((Vowel::Vɑː, "bath: Ɑː -> ɑː (CMUdict)")),
      (Vowel::Vəʊ, Glyph::Ah) => Some((Vowel::Vɒ, "əʊ -> ɒ (CMUdict)")),
      (Vowel::Vaɪ, Glyph::Ee) if s.stress != Stress::Unstressed => Some((Vowel::Viː, "aɪ -> iː (CMUdict)")),
      (Vowel::Və, Glyph::Eh) if ends_ary && i + 2 == n => Some((Vowel::Ve, "-ary: ə -> e (CMUdict)")),
      (Vowel::Və, Glyph::Aw) if ends_ary && i + 2 == n => Some((Vowel::Vɔː, "-ory: ə -> ɔː (CMUdict)")),
      _ => None
    };
    if let Some((vowel, rule)) = edit {
      edits.push((i, vowel, rule));
    }
  }
  edits
}

// No y after an alveolar starting a stressed syllable: ˈdjʊə -> ˈdʊə.
fn drop_yod(ipa: &str) -> String {
  let re = regex::Regex::new("([ˈˌ][tdszlnθ])j(uː|ʊə)").unwrap();
  re.replace_all(ipa, "${1}${2}").into_owned()
}

fn main() {
  let readlex = read_readlex_top_american5000().unwrap();
  let cmu: HashMap<String, Vec<Glyph>> = feature_refining::dictionary::load_dictionary().unwrap().words.into_iter()
    .map(|w| (encode(&w.spelling), w.pronunciation)).collect();
  let tweaks = load_pronunciation_tweaks();
  let overrides: HashMap<&str, &str> = OVERRIDES.iter().cloned().collect();

  let mut out = String::new();
  let mut report = String::new();
  // rule -> (count, examples)
  let mut by_rule: BTreeMap<&'static str, (usize, Vec<String>)> = BTreeMap::new();
  let mut changed_from_before = 0;

  for entry in &readlex {
    let word = entry.latin.as_str();
    let before = tweaks.get(word).cloned().unwrap_or(entry.ipa.clone());

    let after = if let Some(ipa) = overrides.get(word) {
      ipa.to_string()
    }
    else {
      match (parse_ipa(&entry.ipa), parse_ipa(&before)) {
        (Ok((_, original)), Ok((_, syllables))) if original.len() == syllables.len() && syllables.len() > 1 => {
          let n = syllables.len();
          let mut replacements = vec![];
          for i in 0 .. n {
            let v = original[i].vowel;
            if original[i].stress == Stress::Unstressed && (v == Vowel::Vɪ || v == Vowel::VI) {
              let (vowel, rule) = decide(word, i, n, &syllables[i]);
              if vowel != syllables[i].vowel {
                replacements.push((i, vowel));
              }
              let e = by_rule.entry(rule).or_insert((0, vec![]));
              e.0 += 1;
              if e.1.len() < 8 {
                e.1.push(word.to_owned());
              }
            }
          }
          replace_vowels(&before, &replacements)
        },
        // Monosyllables, and the rare hand correction that changes the
        // number of syllables, are kept as they are.
        _ => before.clone()
      }
    };
    let after = if overrides.contains_key(word) {
      after
    }
    else {
      let edits = american_edits(word, &after, cmu.get(word));
      for (_, _, rule) in &edits {
        let e = by_rule.entry(rule).or_insert((0, vec![]));
        e.0 += 1;
        if e.1.len() < 12 {
          e.1.push(word.to_owned());
        }
      }
      let replacements: Vec<(usize, Vowel)> = edits.iter().map(|(i, v, _)| (*i, *v)).collect();
      let edited = if replacements.is_empty() { after.clone() } else { replace_vowels(&after, &replacements) };
      let american = drop_yod(&edited);
      if american != edited {
        let e = by_rule.entry("yod dropped after an alveolar (stressed)").or_insert((0, vec![]));
        e.0 += 1;
        e.1.push(word.to_owned());
      }
      american
    };

    // Count a word as changed if its glyphs changed (i and iː, for example,
    // are the same glyph).
    let glyphs_before = ipa_to_pronunciation_glyphs(&before).unwrap();
    let glyphs_after = ipa_to_pronunciation_glyphs(&after).unwrap();
    if glyphs_before != glyphs_after {
      changed_from_before += 1;
      let _ = writeln!(report, "{:16} {:20} -> {}", word, before, after);
    }
    let _ = writeln!(out, "{} {}", word, after);
  }

  std::fs::write("res/pronunciations-phonemic-ipa.txt", out).unwrap();
  let mut summary = format!("{} words, {} changed from before (ReadLex plus hand corrections)\n\nChanges by rule:\n", readlex.len(), changed_from_before);
  for (rule, (count, examples)) in &by_rule {
    let _ = writeln!(summary, "  {:5} {:50} e.g. {}", count, rule, examples.join(", "));
  }
  std::fs::write("working/phonemic-changes.txt", summary.clone() + "\nChanges:\n" + &report).unwrap();
  print!("{}", summary);
  println!("\nWrote res/pronunciations-phonemic-ipa.txt and working/phonemic-changes.txt");
}
