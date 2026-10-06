# Joint pair search (plan)

Status: planned, not implemented.

## Why

The second pass improves the rule set one half-rule at a time (single-rule
sweeps) and, so far, with "tuple" moves that empty a set of slots and refill
them one at a time. The refill order makes collapsing the natural outcome:
a slot searched while its partner is empty prefers to do the whole job itself,
so an anterior/posterior pair like `[ng]→{0}`, `[{0}]→ŋ` becomes `[ng]→ŋ` plus
a freed slot. That flattens the structure of interacting rules the first pass
builds, before that structure has been exploited.

A joint pair search instead keeps both rules and searches over changes to the
two of them at once. It can find improvements that need both rules to change
together, which a single-rule sweep can't reach, because changing either rule
alone makes things worse.

## The move

A pair move acts on two positions k < j. Both keep a rule afterwards (neither
is emptied, so nothing collapses). A candidate is a pair of half-rules (a, b)
that replaces rules k and j together.

- **Score:** exact, as in `exact_scoring`: for each word, apply rules 0..k-1,
  then a, then rules k+1..j-1, then b, then rules j+1.., and compare with the
  pronunciation.
- **Search baseline:** neither rule present, as the single-rule search uses
  "no rule at k". The current pair is one of the candidates.
- **Acceptance:** the best pair the search finds is taken if it improves the
  global score (`half_rules::global_score`) over the current pair. The
  checkpoint is saved on every accepted change.

## Which pairs

Pairs of rules connected through a token: rule j reads (in `pre`, `at` or
`post`) a token that rule k outputs, the same relation `partners` uses today.
That covers an anterior with its posterior and anteriors that consume another
anterior's token, which is where interactions happen.

## Generating candidates

Candidate generation is per word, like the single-rule search's
`find_improving_edits`, but b is generated in the context a creates:

1. **a candidates:** spans of the word's input at k, with outputs taken from
   the word's targets at later stages, within the output window, one side a
   single glyph. The current rule k is always added.
2. **For each a:** apply a to the input at k, run rules k+1..j-1, giving the
   word's state at j.
3. **b candidates:** spans of that state, with outputs from the targets at
   stages after j, within the window. The current rule j is always added.
4. **Keep** the (a, b) combinations that improve this word over the baseline.

Always including the current rules makes the pair search a superset of the
single-rule moves at k and j: (current a, new b) and (new a, current b) are
among the candidates. It also keeps "worse than the current pair" meaningful as
a sign that the search stopped too early, since the current pair is a
candidate whenever its rules apply to the word.

The targets are the path-consistent targets computed once per sweep. Targets
at stages after j don't depend on rules k or j, so they're exact for b; the
ones used for a reflect the current rule j, which is only a source of
candidate outputs, since scoring is exact.

## Cost and controls

The number of combinations per word is |A| × |B|. With output window 0 and
short (token-compressed) words, that is on the order of hundreds by hundreds,
so up to tens of thousands per introduced word before the "improves this word"
filter. Only words the search introduces pay this. Controls, in order of
preference:

- The output window (already exists), for both a and b.
- A cap on combinations kept per word (best improvements first), if needed.
- Caching the state at j per (word, a output), and the exact score per (word,
  state after b), the way single-rule scoring caches per output.

## Search and parallelism

- `genastarlike` with the edit type `(HalfRule, HalfRule)`, via a new
  `EditSystem` (`PairEditSystem`) holding the inputs at k, the rules between
  and after, the targets and the caches.
- Width: the same adaptive width as single-rule searches. A search that
  returns a pair worse than the current pair is repeated wider.
- Pair searches at different (k, j) are independent, so they run under the
  existing speculative parallel sweep (`tuple_sweep_parallel`'s scheme): one
  pair search per thread, taking the earliest improving pair in sweep order.

## Where it goes in the pipeline

Each round: single-rule sweep, then joint pair sweep. The current
empty-and-refill tuple sweep stays available but is off by default.

## Testing

- The pair search's exact score for a pair equals `global_score` with that
  pair in place.
- A constructed case where changing either rule alone makes things worse but
  changing both together improves the score: the single-rule sweep finds
  nothing and the pair sweep finds the change.
- With adaptive widths off, the parallel pair sweep gives the same result as
  the sequential one.
- On the 300-word set: cost per sweep and final score, against singles plus
  the current tuple sweep.

## Possible later addition: new token pairs

A cheaper, structured subset: a = `pre[at]post → {t}` with a fresh token t,
and b = `[{t}] → content` with the content taken from the target span aligned
with the key. That inserts a first-pass-style rule into two existing slots,
building new structure instead of only adjusting existing structure. Its cost
is about that of single-rule candidate generation, since b is determined by
a's alignment.
