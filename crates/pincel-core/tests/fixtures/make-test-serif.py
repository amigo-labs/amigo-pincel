"""Build the Pincel Test Serif fixtures from Liberation Serif 2.1.5.

Subsets each face to printable ASCII, renames the family (OFL: a modified
version must not use the Reserved Font Names "Liberation", "Tinos") and
packs Regular + Bold into a collection. Needs fontTools.

usage: python make-test-serif.py <liberation-fonts-ttf-2.1.5 dir> <out dir>
"""
import os
import sys

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.ttLib.ttCollection import TTCollection

FAMILY = "Pincel Test Serif"
PS = "PincelTestSerif"
STYLES = ["Regular", "Bold", "Italic", "BoldItalic"]
STYLE_NAMES = {"Regular": "Regular", "Bold": "Bold", "Italic": "Italic", "BoldItalic": "Bold Italic"}


def build(src_dir, out_dir):
    faces = {}
    for style in STYLES:
        font = TTFont(os.path.join(src_dir, f"LiberationSerif-{style}.ttf"))
        opts = subset.Options()
        opts.layout_features = ["*"]
        opts.hinting = False
        opts.name_IDs = [0, 1, 2, 3, 4, 5, 6, 13, 14]
        opts.notdef_outline = True
        sub = subset.Subsetter(opts)
        sub.populate(unicodes=range(0x20, 0x7F))
        sub.subset(font)
        name = font["name"]
        sub_name = STYLE_NAMES[style]
        full = FAMILY if style == "Regular" else f"{FAMILY} {sub_name}"
        for rec in list(name.names):
            if rec.nameID == 1:
                rec.string = FAMILY
            elif rec.nameID == 3:
                rec.string = f"{PS}-{style}; Pincel test fixture"
            elif rec.nameID == 4:
                rec.string = full
            elif rec.nameID == 6:
                rec.string = f"{PS}-{style}"
            elif rec.nameID == 5:
                rec.string = "Version 2.01.5; Pincel test fixture subset of Liberation Serif 2.1.5"
        if "CFF " in font:
            raise SystemExit("unexpected CFF")
        path = os.path.join(out_dir, f"{PS}-{style}.ttf")
        font.save(path)
        faces[style] = path
    ttc = TTCollection()
    ttc.fonts = [TTFont(faces["Regular"]), TTFont(faces["Bold"])]
    ttc.save(os.path.join(out_dir, f"{PS}.ttc"))


if __name__ == "__main__":
    build(sys.argv[1], sys.argv[2])
