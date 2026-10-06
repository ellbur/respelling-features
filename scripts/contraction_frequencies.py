#!/usr/bin/env python3
# Generates res/contraction-frequencies.txt.
#
# res/topwords.txt (COCA) splits contractions into pieces ("do" + "n't"), and
# ReadLex gives them frequency 0 for the same reason, so neither can say how
# common "don't" is. wordfreq keeps contractions whole. This takes every
# ReadLex word containing an apostrophe, plus common possessives and "'ve"
# forms of ReadLex words, looks up their wordfreq frequencies, and
# rescales it into topwords.txt's units using the median ratio over words that
# appear in both lists.
#
# Requires the wordfreq package. Run from the repository root.

import json
import statistics
import wordfreq

def load_topwords():
    counts = {}
    with open('res/topwords.txt') as f:
        next(f)
        for line in f:
            tokens = line.split(' ')
            counts[tokens[1]] = int(tokens[2])
    return counts

def main():
    topwords = load_topwords()

    ratios = [
        count / wordfreq.word_frequency(word, 'en')
        for word, count in topwords.items()
        if "'" not in word and wordfreq.word_frequency(word, 'en') > 0
    ]
    scale = statistics.median(ratios)

    readlex = json.load(open('res/readlex.json'))
    words = sorted({
        e['Latn']
        for entries in readlex.values()
        for e in entries
        if "'" in e['Latn'] and ' ' not in e['Latn'] and '-' not in e['Latn']
    })

    # Possessives ("women's") and "would've"-style forms aren't in ReadLex, but
    # their pronunciations follow from the base word, which the dictionary
    # builder derives. List the common ones whose base word ReadLex has.
    readlex_words = {e['Latn'] for entries in readlex.values() for e in entries}
    for word in wordfreq.top_n_list('en', 50000):
        for suffix in ("'s", "'ve"):
            if word.endswith(suffix) and word not in readlex_words and word[:-len(suffix)] in readlex_words:
                words.append(word)

    # wordfreq strips leading and trailing apostrophes ("'em" becomes "em") and
    # splits some words ("d'you" becomes "d" + "you"); its frequencies for
    # those are not the word's, so keep only words it treats as one token.
    words = sorted(word for word in words if wordfreq.tokenize(word, 'en') == [word])

    with open('res/contraction-frequencies.txt', 'w') as out:
        for word in words:
            count = round(wordfreq.word_frequency(word, 'en') * scale)
            if count > 0:
                out.write(f'{word} {count}\n')

    print(f'scale = {scale:.4g}, {len(ratios)} words used')

if __name__ == '__main__':
    main()
