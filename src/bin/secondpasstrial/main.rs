// Runs the first pass on the most common words, then second-pass sweeps until
// one changes nothing, printing what each sweep did.
//
// The first-pass rules are saved to working/first-pass-<words>-<rules>.txt and
// reused on later runs with the same sizes.

use clap::Parser;
use std::time::Instant;

use feature_refining::dictionary;
use feature_refining::first_pass::FirstPass;
use feature_refining::gaussian_astarlike22::GaussianSystem;
use feature_refining::genastarlike::Outcome;
use feature_refining::half_rules::flatten;
use feature_refining::high_level_substitutions2::HLSubstitutionList;
use feature_refining::second_pass::{SecondPass, SweepStats};

#[derive(Parser)]
struct Args {
  #[arg(long, default_value_t = 1000)]
  words: usize,
  #[arg(long, default_value_t = 100)]
  rules: usize,
  #[arg(long, default_value_t = 5)]
  max_sweeps: usize,
  // Print each change a sweep makes.
  #[arg(long)]
  verbose: bool,
  // Score pass-2 candidates by running to the final output.
  #[arg(long)]
  exact: bool,
  // Take pass-2 candidate outputs from every later stage's target.
  #[arg(long)]
  all_stages: bool,
  // After each single-rule sweep, re-optimize rules in pairs.
  #[arg(long)]
  pairs: bool,
  // The pass-2 Gaussian estimator width (starting width if --adaptive).
  #[arg(long, default_value_t = 4.0)]
  scale: f64,
  // Adapt the width: widen and retry when a search's rule fails the global
  // check, narrow when it passes.
  #[arg(long)]
  adaptive: bool,
  // Stop each position's search as soon as a rule beats the current one.
  #[arg(long)]
  early_stop: bool,
  // Take equally good rules whose output has fewer tokens.
  #[arg(long)]
  direct_ties: bool,
  // Only take pass-2 candidate outputs from within this many glyphs of where
  // the key lines up with the target.
  #[arg(long)]
  window: Option<usize>,
  // When a round changes nothing, widen the window by one, up to this.
  #[arg(long)]
  window_max: Option<usize>,
  // Pass-2 search batch sizes: candidates checked at once, and words each is
  // checked against per batch.
  #[arg(long, default_value_t = 256)]
  edit_chunk: usize,
  #[arg(long, default_value_t = 256)]
  steps_chunk: usize,
  // With --pairs: when pairs find nothing, try triples, and so on up to
  // tuples of this size.
  #[arg(long, default_value_t = 2)]
  max_tuple: usize,
  // Don't cache exact scores.
  #[arg(long)]
  no_memo: bool,
  // Run tuple sweeps on this many threads at once, one tuple per thread (0:
  // sequential).
  #[arg(long, default_value_t = 0)]
  tuple_threads: usize,
  // Which dictionary to train on: cmudict (the original), readlex (ReadLex
  // with American pronunciations, tweaks, contractions and possessives), or
  // readlex-phonemic (the same words with consistent unstressed vowels).
  #[arg(long, default_value = "cmudict")]
  dictionary: String,
  // Write the final rules here.
  #[arg(long)]
  output: Option<String>,
  // Save pass 2's rules here after every round, and resume from them if the
  // file exists.
  #[arg(long)]
  checkpoint: Option<String>,
  // Run the first pass's searches with pargenastarlike on this many threads
  // (0: genastarlike).
  #[arg(long, default_value_t = 0)]
  first_pass_threads: usize,
  // The first pass's Gaussian estimator width.
  #[arg(long, default_value_t = 4.0)]
  first_pass_scale: f64,
  // Print a progress line every this many positions of a pass-2 sweep.
  #[arg(long, default_value_t = 0)]
  progress: usize,
  // With --adaptive: the factors for a failed search and an accepted change.
  #[arg(long, default_value_t = 1.2)]
  widen: f64,
  #[arg(long, default_value_t = 0.9)]
  narrow: f64,
  // With --adaptive: the earlier scheme instead, doubling and searching a
  // failed position again.
  #[arg(long)]
  adaptive_retry: bool,
  // Tuple moves search jointly over all the tuple's rules, keeping them all,
  // instead of emptying and refilling the slots.
  #[arg(long)]
  joint: bool,
  // For --joint: partial tuples kept per slot in candidate generation.
  #[arg(long, default_value_t = 32)]
  beam: usize,
  // For --joint: judge partial tuples optimistically (the best option at
  // the next slot).
  #[arg(long)]
  optimistic: bool,
  // With --pairs: first optimistic joint pairs until they converge, then
  // refill tuples (up to --max-tuple) until those converge.
  #[arg(long)]
  then_refill: bool,
  // Don't make rules anchored to the start (^) or end ($) of a word, in
  // either pass. (Their ignore rules slow shaping.)
  #[arg(long)]
  no_anchors: bool,
  // Don't make rules with lookbehind or lookahead context, in either pass.
  #[arg(long)]
  no_context: bool,
  // Allow anti-anchored rules (~: kept from the start or end of a word by a
  // word glyph in the context), in both passes.
  #[arg(long)]
  anti_anchors: bool,
  // Once pass 2 has converged, broaden rules' context into glyph sets (see
  // SecondPass::broaden_sweep) until that converges too.
  #[arg(long)]
  broaden: bool
}

fn main() {
  let args = Args::parse();
  let system = GaussianSystem { scale: args.first_pass_scale };

  let mut dictionary = match args.dictionary.as_str() {
    "cmudict" => dictionary::load_dictionary().unwrap(),
    "readlex" => feature_refining::readlex::load_readlex_ipa_american_tweaked_dictionary().unwrap(),
    "readlex-phonemic" => feature_refining::readlex::load_readlex_phonemic_dictionary().unwrap(),
    other => panic!("Unknown dictionary {}", other)
  };
  dictionary.words.truncate(args.words);
  // Scale frequencies so the most common word has frequency 1, as in the
  // cmudict dictionary; the size cost and the Gaussian width assume that.
  let max_frequency = dictionary.words.iter().map(|w| w.frequency).fold(0.0, f64::max);
  for w in dictionary.words.iter_mut() {
    w.frequency /= max_frequency;
  }
  println!("{} words from {}", dictionary.words.len(), args.dictionary);

  // The cmudict files, and files for the default width, keep their original
  // names.
  let scale_suffix = if args.first_pass_scale == 4.0 { String::new() } else { format!("-scale{}", args.first_pass_scale) }
    + if args.no_anchors { "-noanchors" } else { "" }
    + if args.no_context { "-nocontext" } else { "" }
    + if args.anti_anchors { "-anti" } else { "" };
  let path = match args.dictionary.as_str() {
    "cmudict" => format!("working/first-pass-{}-{}{}.txt", args.words, args.rules, scale_suffix),
    other => format!("working/first-pass-{}-{}-{}{}.txt", other, args.words, args.rules, scale_suffix)
  };
  let first_rules = match std::fs::read_to_string(&path) {
    Ok(text) => {
      println!("Using first-pass rules from {}", path);
      HLSubstitutionList::decode(&text).unwrap()
    },
    Err(_) => {
      // Progress is saved after every rule, and picked up from there if the
      // run is interrupted.
      let partial_path = format!("{}.partial", path);
      let encode = |rules: &HLSubstitutionList| -> String {
        let text: Vec<String> = rules.substitutions.iter().map(|s| s.encode()).collect();
        text.join("\n") + "\n"
      };
      let mut first = match std::fs::read_to_string(&partial_path) {
        Ok(text) => {
          let rules = HLSubstitutionList::decode(&text).unwrap();
          println!("Resuming the first pass from {} ({} rules)", partial_path, rules.substitutions.len());
          FirstPass::resume(&dictionary, &rules)
        },
        Err(_) => FirstPass::setup(&dictionary, HLSubstitutionList { substitutions: vec![] })
      };
      first.allow_anchors = !args.no_anchors;
      first.allow_context = !args.no_context;
      first.allow_anti_anchors = args.anti_anchors;
      let start = Instant::now();
      for i in first.rules.substitutions.len() .. args.rules {
        let rule_start = Instant::now();
        let outcome = if args.first_pass_threads > 0 {
          first.find_next_rule_parallel(&system, feature_refining::pargenastarlike::ParParams::with_threads(args.first_pass_threads))
        }
        else {
          first.find_next_rule(&system, false)
        };
        match outcome {
          Outcome::FoundImprovement(sub, _) => println!("first pass {:>4}: {} ({:.1}s)", i, sub.encode(), rule_start.elapsed().as_secs_f64()),
          _ => { println!("First pass stopped early: no improvement."); break; }
        }
        std::fs::write(&partial_path, encode(&first.rules)).unwrap();
      }
      println!("First pass took {:.1}s", start.elapsed().as_secs_f64());
      std::fs::write(&path, encode(&first.rules)).unwrap();
      let _ = std::fs::remove_file(&partial_path);
      first.rules
    }
  };

  let (half_rules, checkpoint_scale, checkpoint_position, checkpoint_tuple, checkpoint_skip, checkpoint_broadening) = match args.checkpoint.as_ref().and_then(|path| SecondPass::load_checkpoint(path)) {
    Some(c) => {
      println!("Resuming pass 2 from {} ({} half-rules, sweep at position {}, tuple sweep {:?}{}{})", args.checkpoint.as_ref().unwrap(), c.rules.len(), c.position, c.tuple_position,
        if c.tuple_skip > 0 { format!(" after {} of its tuples", c.tuple_skip) } else { String::new() },
        if c.broadening { ", broadening" } else { "" });
      (c.rules, c.scale, c.position, c.tuple_position, c.tuple_skip, c.broadening)
    },
    None => (flatten(&first_rules), None, 0, None, 0, false)
  };
  
  let mut pass = SecondPass::new(&dictionary, half_rules);
  pass.options.exact_scoring = args.exact;
  pass.options.outputs_from_all_stages = args.all_stages;
  pass.options.early_stop = args.early_stop;
  pass.options.accept_direct_ties = args.direct_ties;
  pass.options.output_window = args.window;
  pass.options.edit_chunk_size = args.edit_chunk;
  pass.options.steps_chunk_size = args.steps_chunk;
  pass.options.memoize = !args.no_memo;
  pass.options.joint_tuples = args.joint;
  pass.options.beam = args.beam;
  pass.options.optimistic_beam = args.optimistic || args.then_refill;
  if args.then_refill {
    pass.options.joint_tuples = true;
  }
  pass.scale = checkpoint_scale.unwrap_or(args.scale);
  pass.sweep_position = checkpoint_position;
  pass.tuple_position = checkpoint_tuple;
  pass.tuple_skip = checkpoint_skip;
  pass.broadening = checkpoint_broadening;
  pass.options.allow_anchors = !args.no_anchors;
  pass.options.allow_context = !args.no_context;
  pass.options.allow_anti_anchors = args.anti_anchors;
  pass.progress_every = args.progress;
  // Saved whenever the rules change.
  pass.checkpoint_path = args.checkpoint.clone();
  if args.adaptive {
    pass.adaptive = Some(if args.adaptive_retry {
      feature_refining::second_pass::AdaptiveScale::per_position()
    }
    else {
      feature_refining::second_pass::AdaptiveScale { widen: args.widen, narrow: args.narrow, ..Default::default() }
    });
  }
  println!("{} half-rules, global score {:.4}", pass.rules.len(), pass.score());

  // Broadening comes after the rest of pass 2 has converged.
  let pass2_sweeps = if pass.broadening { 0 } else { args.max_sweeps };
  for sweep in 0 .. pass2_sweeps {
    let start = Instant::now();
    // A run resumed in the middle of a tuple sweep goes straight back to it;
    // that round's single-rule sweep was already done.
    let resumed_tuples = pass.tuple_position.map(|(n, _)| n);
    let stats = if resumed_tuples.is_some() {
      let score = pass.score();
      println!("sweep {}: skipped (resuming a tuple sweep)", sweep);
      SweepStats { score_before: score, score_after: score, final_scale: pass.scale, ..Default::default() }
    }
    else {
      pass.sweep(args.verbose)
    };
    println!(
      "sweep {}: {:.4} -> {:.4} ({:+.2}%), {} replaced, {} removed, {} positions with unreachable words ({} word-positions), {:.1}s",
      sweep, stats.score_before, stats.score_after,
      100.0 * (stats.score_after - stats.score_before) / stats.score_before,
      stats.replaced, stats.removed,
      stats.positions_with_unreachable, stats.total_unreachable,
      start.elapsed().as_secs_f64()
    );
    println!(
      "  search found: current rule {}, equivalent rule {} ({} taken as more direct), other rule (rejected) {}, other rule (accepted) {}, nothing (rule kept) {}, nothing (rule removed) {}; {} retries, scale now {:.3}",
      stats.search_found_current, stats.search_found_equivalent + stats.direct_ties_taken, stats.direct_ties_taken,
      stats.search_found_other_rejected, stats.replaced - stats.direct_ties_taken,
      stats.search_found_nothing_kept, stats.removed, stats.retries, stats.final_scale
    );
    if args.verbose {
      for (k, current, candidate, change) in &stats.rejected {
        println!("  rejected at {:>4}: {:?} -> {:?} would change score by {:+.4}", k, current, candidate, change);
      }
    }
    let mut pair_changes = 0;
    if args.pairs {
      // Larger tuples only once the smaller ones find nothing. Joint pairs
      // (the first phase of --then-refill) are pairs only.
      let max_tuple = if pass.options.joint_tuples && args.then_refill { 2 } else { args.max_tuple };
      for n in resumed_tuples.unwrap_or(2) ..= max_tuple {
        let start = Instant::now();
        let before = pass.score();
        pair_changes = if args.tuple_threads > 0 {
          pass.tuple_sweep_parallel(n, args.tuple_threads, args.verbose).len()
        }
        else {
          pass.tuple_sweep(n, args.verbose).len()
        };
        let after = pass.score();
        println!(
          "{}-tuple sweep {}: {:.4} -> {:.4} ({:+.2}%), {} changed, {} half-rules now, {:.1}s",
          n, sweep, before, after, 100.0 * (after - before) / before, pair_changes, pass.rules.len(), start.elapsed().as_secs_f64()
        );
        if pair_changes > 0 {
          break;
        }
      }
    }
    // A resumed round didn't include its single-rule sweep's changes, so it
    // can't show convergence.
    if resumed_tuples.is_some() {
      continue;
    }
    if stats.replaced == 0 && stats.removed == 0 && pair_changes == 0 && args.then_refill && pass.options.joint_tuples {
      pass.options.joint_tuples = false;
      println!("joint pairs converged at {:.4}; switching to refill tuples", pass.score());
      continue;
    }
    if stats.replaced == 0 && stats.removed == 0 && pair_changes == 0 {
      match (pass.options.output_window, args.window_max) {
        (Some(window), Some(max)) if window < max => {
          pass.options.output_window = Some(window + 1);
          println!("window widened to {}", window + 1);
        }
        _ => break
      }
    }
  }

  if args.broaden {
    for sweep in 0 .. args.max_sweeps {
      let start = Instant::now();
      let stats = pass.broaden_sweep(args.verbose);
      println!(
        "broaden sweep {}: {:.4} -> {:.4} ({:+.2}%), {} broadened, {} rejected, scale now {:.3}, {:.1}s",
        sweep, stats.score_before, stats.score_after,
        100.0 * (stats.score_after - stats.score_before) / stats.score_before,
        stats.replaced, stats.search_found_other_rejected, stats.final_scale, start.elapsed().as_secs_f64()
      );
      if stats.replaced == 0 {
        break;
      }
    }
  }

  if let Some(output) = &args.output {
    let text: Vec<String> = pass.rules.iter().map(|r| r.encode()).collect();
    std::fs::write(output, text.join("\n") + "\n").unwrap();
    println!("Wrote {} rules to {}", pass.rules.len(), output);
  }
  
  println!("Final rules:");
  for r in &pass.rules {
    println!("  {:?}", r);
  }
}
