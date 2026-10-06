// Levenshtein distance with the usual two-row dynamic program. The words here
// are short, so this is much faster than levenshtein_diff's memoized
// recursion, and it gives the same distances.

pub fn distance<T: PartialEq>(a: &[T], b: &[T]) -> u32 {
  if a.is_empty() { return b.len() as u32; }
  if b.is_empty() { return a.len() as u32; }

  const INLINE: usize = 64;
  let mut prev_buf = [0u32; INLINE];
  let mut curr_buf = [0u32; INLINE];
  let mut prev_vec;
  let mut curr_vec;
  let (mut prev, mut curr): (&mut [u32], &mut [u32]) = if b.len() < INLINE {
    (&mut prev_buf[.. b.len() + 1], &mut curr_buf[.. b.len() + 1])
  }
  else {
    prev_vec = vec![0u32; b.len() + 1];
    curr_vec = vec![0u32; b.len() + 1];
    (&mut prev_vec[..], &mut curr_vec[..])
  };

  for j in 0 ..= b.len() {
    prev[j] = j as u32;
  }

  for i in 1 ..= a.len() {
    curr[0] = i as u32;
    for j in 1 ..= b.len() {
      let substitution = prev[j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 };
      curr[j] = substitution.min(prev[j] + 1).min(curr[j - 1] + 1);
    }
    std::mem::swap(&mut prev, &mut curr);
  }

  prev[b.len()]
}

#[cfg(test)]
mod tests {
  use super::*;
  use rand::{Rng, SeedableRng};

  #[test]
  fn matches_levenshtein_diff() {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(0);
    for _ in 0 .. 20000 {
      let alphabet = rng.gen_range(1 ..= 4u8);
      let a: Vec<u8> = (0 .. rng.gen_range(0 .. 12)).map(|_| rng.gen_range(0 .. alphabet)).collect();
      let b: Vec<u8> = (0 .. rng.gen_range(0 .. 12)).map(|_| rng.gen_range(0 .. alphabet)).collect();
      let expected = levenshtein_diff::distance(&a, &b).0 as u32;
      assert_eq!(distance(&a, &b), expected, "{:?} {:?}", a, b);
    }
  }

  #[test]
  fn long_inputs() {
    let a: Vec<u8> = (0 .. 100).map(|i| (i % 7) as u8).collect();
    let b: Vec<u8> = (0 .. 90).map(|i| (i % 5) as u8).collect();
    assert_eq!(distance(&a, &b), levenshtein_diff::distance(&a, &b).0 as u32);
  }
}
