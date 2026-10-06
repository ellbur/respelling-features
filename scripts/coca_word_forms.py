#!/usr/bin/env python3
# Converts the "4 forms" sheet of wordfrequency.info's free COCA sample
# (wordFrequency.xlsx: the top 5,000 word forms) into res/topwords.txt: one
# line per word, space-separated, with %caps and the per-million columns to
# two decimals. Standard library only.
#
#   coca_word_forms.py <wordFrequency.xlsx> <topwords.txt>

import re
import sys
import zipfile
import xml.etree.ElementTree as ET

NS = {'m': 'http://schemas.openxmlformats.org/spreadsheetml/2006/main',
      'r': 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'}
REL_NS = '{http://schemas.openxmlformats.org/package/2006/relationships}'


def text(el):
    return ''.join(t.text or '' for t in el.iter(f"{{{NS['m']}}}t"))


def sheet_rows(xlsx, name):
    z = zipfile.ZipFile(xlsx)
    workbook = ET.fromstring(z.read('xl/workbook.xml'))
    rel_id = next(s.get(f"{{{NS['r']}}}id") for s in workbook.iter(f"{{{NS['m']}}}sheet") if s.get('name').startswith(name))
    rels = ET.fromstring(z.read('xl/_rels/workbook.xml.rels'))
    target = next(r.get('Target') for r in rels.iter(f'{REL_NS}Relationship') if r.get('Id') == rel_id)
    strings = [text(si) for si in ET.fromstring(z.read('xl/sharedStrings.xml')).findall('m:si', NS)]
    sheet = ET.fromstring(z.read('xl/' + target.lstrip('/').removeprefix('xl/')))
    for row in sheet.iter(f"{{{NS['m']}}}row"):
        values = []
        for c in row.findall('m:c', NS):
            v = c.find('m:v', NS)
            kind = c.get('t')
            if kind == 's':
                values.append(strings[int(v.text)])
            elif kind == 'inlineStr':
                values.append(text(c))
            elif kind == 'b':
                # The words "true" and "false", which Excel took for booleans.
                values.append('TRUE' if v.text == '1' else 'FALSE')
            else:
                values.append(v.text if v is not None else '')
        yield values


def main():
    xlsx, out = sys.argv[1:]
    rows = sheet_rows(xlsx, '4 forms')
    header = next(rows)
    two_decimals = [h == '%caps' or h.endswith('PM') for h in header]
    with open(out, 'w') as f:
        f.write(' '.join(header) + '\n')
        for row in rows:
            f.write(' '.join('%.2f' % float(v) if d else v for v, d in zip(row, two_decimals)) + '\n')


if __name__ == '__main__':
    main()
