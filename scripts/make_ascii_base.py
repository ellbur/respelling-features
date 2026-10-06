#!/usr/bin/env python3
# Makes the base for the ASCII respelling font from an ordinary TrueType
# font (DejaVu Sans by default):
#
# - keeps only printable ASCII, to keep the web font small;
# - gives the apostrophe a glyph named "apos", as the rules call it;
# - adds the glyphs the rules use but never show (tokens syn0 .. syn<N-1>
#   and the sound glyphs, which the respelling turns into letters) as empty,
#   zero-width glyphs;
# - renames the family, as DejaVu's (Bitstream Vera) license requires of a
#   modified font.
#
#   make_ascii_base.py <output.ttf> --syn N [--input DejaVuSans.ttf] [--family "Heelee ASCII"]
#
# Requires fontTools.

import argparse
import copy

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._g_l_y_f import Glyph

# Glyph::name for every sound glyph that isn't a letter.
SOUND_GLYPHS = ['ch', 'th', 'sh', 'jh', 'eh', 'ah', 'oi', 'ow', 'aw', 'eu', 'uh', 'ee', 'ei',
                'yu', 'dh', 'ng', 'ae', 'ih', 'schwa', 'er']


def add_glyph(font, name, glyph, advance, lsb):
    glyf = font['glyf']
    glyf.glyphs[name] = glyph
    glyf.glyphOrder = list(glyf.glyphOrder) + [name]
    font['hmtx'].metrics[name] = (advance, lsb)
    font.setGlyphOrder(glyf.glyphOrder)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output')
    parser.add_argument('--syn', type=int, default=0)
    parser.add_argument('--input', default='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
    parser.add_argument('--family', default='Heelee ASCII')
    args = parser.parse_args()

    font = TTFont(args.input)
    options = subset.Options()
    options.glyph_names = True
    options.name_IDs = ['*']
    options.layout_features = ['kern']
    options.notdef_outline = True
    subsetter = subset.Subsetter(options)
    subsetter.populate(unicodes=range(0x20, 0x7f))
    subsetter.subset(font)

    glyf = font['glyf']
    advance, lsb = font['hmtx'].metrics['quotesingle']
    add_glyph(font, 'apos', copy.deepcopy(glyf['quotesingle']), advance, lsb)
    for table in font['cmap'].tables:
        if 0x27 in table.cmap:
            table.cmap[0x27] = 'apos'

    for name in SOUND_GLYPHS + [f'syn{n}' for n in range(args.syn)]:
        if name not in glyf:
            empty = Glyph()
            empty.numberOfContours = 0
            add_glyph(font, name, empty, 0, 0)

    font['maxp'].numGlyphs = len(font.getGlyphOrder())
    font['post'].formatType = 2.0

    for record in font['name'].names:
        if record.nameID in (1, 4, 16):
            record.string = args.family
        elif record.nameID == 3:
            record.string = f'{args.family}; generated'
        elif record.nameID == 6:
            record.string = args.family.replace(' ', '')
    font.save(args.output)
    print(f'{args.output}: {len(font.getGlyphOrder())} glyphs')


if __name__ == '__main__':
    main()
