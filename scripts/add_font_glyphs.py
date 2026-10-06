#!/usr/bin/env python3
# Adds the glyphs the generated features need to a copy of the font:
#
# - token glyphs syn0 .. syn<N-1> that are missing. Tokens only exist between
#   the substitutions and never reach the final output, so they're empty and
#   zero-width.
# - placeholders for phonetic glyphs the font doesn't have yet, copied from
#   an existing glyph, e.g. yu:u draws "yu" as a copy of "u", until real
#   designs replace them.
#
#   add_font_glyphs.py <input.otf> <output.otf> --syn N [--placeholder yu:u ...]
#
# Requires fontTools. Only handles CFF-based (.otf) fonts.

import argparse
from fontTools.ttLib import TTFont
from fontTools.misc.psCharStrings import T2CharString


def add_glyph(font, name, charstring, advance, lsb):
    top = font['CFF '].cff.topDictIndex[0]
    charstrings = top.CharStrings
    # Glyphs read from a binary font are stored by index, which can't be
    # extended; keep them by name instead (as fonts read from TTX are).
    if charstrings.charStringsAreIndexed:
        charstrings.charStrings = {g: charstrings.charStringsIndex[i] for g, i in charstrings.charStrings.items()}
        charstrings.charStringsAreIndexed = 0
    charstrings.charStrings[name] = charstring
    # The charset is the same list as the font's glyph order.
    top.charset.append(name)
    font['hmtx'].metrics[name] = (advance, lsb)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('input')
    parser.add_argument('output')
    parser.add_argument('--syn', type=int, default=0)
    parser.add_argument('--placeholder', action='append', default=[])
    parser.add_argument('--empty', action='append', default=[], help='an extra empty, zero-width glyph')
    args = parser.parse_args()

    font = TTFont(args.input)
    top = font['CFF '].cff.topDictIndex[0]
    charstrings = top.CharStrings
    existing = set(font.getGlyphOrder())
    added = []

    for n in range(args.syn):
        name = f'syn{n}'
        if name not in existing:
            empty = T2CharString(program=[0, 'endchar'], private=top.Private, globalSubrs=top.GlobalSubrs)
            add_glyph(font, name, empty, 0, 0)
            added.append(name)

    for name in args.empty:
        if name not in existing:
            empty = T2CharString(program=[0, 'endchar'], private=top.Private, globalSubrs=top.GlobalSubrs)
            add_glyph(font, name, empty, 0, 0)
            added.append(name)

    for spec in args.placeholder:
        name, source = spec.split(':')
        if name in existing:
            continue
        original = charstrings[source]
        original.decompile()
        copy = T2CharString(program=list(original.program), private=original.private, globalSubrs=original.globalSubrs)
        advance, lsb = font['hmtx'].metrics[source]
        add_glyph(font, name, copy, advance, lsb)
        added.append(name)

    # Same list again, but this resets fontTools' cached name -> index map.
    font.setGlyphOrder(top.charset)
    font['maxp'].numGlyphs = len(font.getGlyphOrder())
    font.save(args.output)
    print(f'added {len(added)} glyphs ({", ".join(added[:3])}{"..." if len(added) > 3 else ""}); {len(font.getGlyphOrder())} in all')


if __name__ == '__main__':
    main()
