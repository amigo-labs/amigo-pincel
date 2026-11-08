#!/usr/bin/env python3
"""Regenerates the golden corpus of pincel-ocr (docs/specs/ocr-recognizer.md).

Renders the line images, runs the reference Tesseract on each (flattened)
image and stores its TSV next to it:

    tests/golden/<lang>/<name>.png   the line image the test recognizes
    tests/golden/<lang>/<name>.tsv   the reference Tesseract's TSV for it
    tests/golden/reference.txt       the reference's version and command

Needs Python 3 with Pillow, the fonts DejaVu Sans, Liberation Serif and
Noto Sans (fontconfig finds them), and the reference Tesseract 5.5 command
line tool. Set TESSERACT to its path if `tesseract` on PATH is another
version. The models come from crates/pincel-ocr/models.

    python3 crates/pincel-ocr/tests/golden/make_corpus.py
"""

import os
import random
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
CRATE = HERE.parent.parent
REPO = CRATE.parent.parent
TESSERACT = os.environ.get("TESSERACT", "tesseract")
ARGS = ["--psm", "7", "--oem", "1", "-c", "lstm_use_matrix=0", "-c", "dotproduct=generic", "tsv"]

FONTS = {"dejavu": "DejaVu Sans", "liberation": "Liberation Serif", "noto": "Noto Sans"}
X_HEIGHTS = [6, 10, 16, 32]

TEXTS = {
    "deu": [
        "Größe: 12,50 € für „Äpfel“ und Öl.",
        "Über 30 Bücher kosten 129,99 €!",
        "Die Straße ist schön, oder?",
        "Änderungen speichern (Strg+S)",
        "Müller & Söhne GmbH, Tel. 0221/4711",
        "„Grüß Gott“, sagte der Bäcker.",
        "Ebene 3 von 12; Deckkraft 85 %",
        "Öffnen, Schließen, Rückgängig",
        "Maße: 1920 × 1080 Pixel",
        "Zürich, Köln und Düsseldorf",
        "Fußgänger überqueren die Brücke.",
        "Bitte wählen Sie eine Datei aus.",
    ],
    "eng": [
        "The quick brown fox jumps over 13 lazy dogs.",
        "Save changes? (Ctrl+S) 42 files",
        "Layer 3 of 12, opacity 85%",
        "“Hello,” she said; ‘goodbye!’",
        "Price: $19.99 per unit, 2 for $35",
        "Export as PNG, JPEG or WebP",
        "Width 1920 px, height 1080 px",
        "Open recent: sprite_sheet.aseprite",
        "Zoom in, zoom out, fit to window",
        "Brightness and contrast: +10/-5",
        "Undo the last 7 steps.",
        "Select a file to continue.",
    ],
}


def font_path(family):
    out = subprocess.run(
        ["fc-match", "-f", "%{file}", family], check=True, capture_output=True, text=True
    ).stdout
    return out.strip()


def ink_box(im):
    """Bounding box (l, t, r, b) of non-background pixels of an L image."""
    return Image.eval(im, lambda v: 255 - v).getbbox()


def font_for_xheight(path, target):
    """The font size whose 'x' is closest to `target` pixels high."""
    best = None
    for size in range(4, 400):
        font = ImageFont.truetype(path, size)
        im = Image.new("L", (size * 2, size * 2), 255)
        ImageDraw.Draw(im).text((size // 2, size // 2), "x", font=font, fill=0)
        box = ink_box(im)
        if box is None:
            continue
        xh = box[3] - box[1]
        if best is None or abs(xh - target) < abs(best[1] - target):
            best = (size, xh)
        if xh > target + 1:
            break
    return best


def render(text, font, pad_x, pad_y, fg=(0, 0, 0), bg=(255, 255, 255)):
    left, top, right, bottom = font.getbbox(text)
    w, h = right - left + 2 * pad_x, bottom - top + 2 * pad_y
    im = Image.new("RGB", (w, h), bg)
    ImageDraw.Draw(im).text((pad_x - left, pad_y - top), text, font=font, fill=fg)
    return im


def render_transparent(text, font, pad):
    left, top, right, bottom = font.getbbox(text)
    w, h = right - left + 2 * pad, bottom - top + 2 * pad
    im = Image.new("RGBA", (w, h), (255, 255, 255, 0))
    ImageDraw.Draw(im).text((pad - left, pad - top), text, font=font, fill=(255, 255, 255, 255))
    return im


def flatten(im):
    """The recognizer's flattening rule, in the same integer arithmetic as
    crates/pincel-ocr/src/image/mod.rs: composite over black when the
    alpha-weighted mean luminance is above 50 %, else over white."""
    im = im.convert("RGBA")
    px = list(im.getdata())
    weighted = sum((299 * r + 587 * g + 114 * b) * a for r, g, b, a in px)
    alpha = sum(a for _, _, _, a in px)
    bg = 0 if weighted * 2 > alpha * 255 * 1000 else 255
    out = Image.new("RGB", im.size)
    out.putdata(
        [
            tuple((c * a + bg * (255 - a) + 127) // 255 for c in (r, g, b))
            for r, g, b, a in px
        ]
    )
    return out


def ui_crops(lang):
    """Crops of Pincel's own UI screenshots: the phone tool bar (German UI)
    and the website's preview image (English text)."""
    bar = Image.open(REPO / ".playwright-mcp" / "phone-bar-start.png").convert("RGB")
    og = Image.open(REPO / "website" / "static" / "og" / "default.png").convert("RGB")
    if lang == "deu":
        return {
            "ui_groesse": bar.crop((0, 8, 110, 36)),
            "ui_form": bar.crop((200, 8, 345, 36)),
            "ui_toolbar": bar.crop((0, 4, 391, 40)),
        }
    return {
        "ui_tagline": og.crop((70, 415, 790, 465)),
        "ui_features": og.crop((70, 470, 640, 512)),
        "ui_domain": og.crop((660, 555, 1130, 592)),
        "ui_title": og.crop((70, 270, 510, 395)),
    }


def lines(lang):
    rng = random.Random(f"pincel-ocr-{lang}")
    texts = TEXTS[lang]
    out = {}
    i = 0
    for key, family in FONTS.items():
        path = font_path(family)
        for xh in X_HEIGHTS:
            size, _ = font_for_xheight(path, xh)
            font = ImageFont.truetype(path, size)
            for k in range(2):
                text = texts[i % len(texts)]
                i += 1
                pad_x = rng.choice([2, 4, 8, 12])
                pad_y = rng.choice([2, 4, 8, 12])
                out[f"{key}_x{xh}_{k}"] = render(text, font, pad_x, pad_y)
    dejavu = font_path(FONTS["dejavu"])
    noto = font_path(FONTS["noto"])
    serif = font_path(FONTS["liberation"])
    f16 = ImageFont.truetype(dejavu, font_for_xheight(dejavu, 16)[0])
    n10 = ImageFont.truetype(noto, font_for_xheight(noto, 10)[0])
    if lang == "deu":
        out["dark_white"] = render("Ebenen zusammenführen", f16, 8, 6, (255, 255, 255), (30, 30, 30))
        out["dark_blue"] = render("Farbe wählen: Blau", n10, 6, 6, (210, 220, 230), (20, 40, 90))
        out["transparent"] = render_transparent("Größe ändern", f16, 6)
        out["single_char"] = render("Ä", f16, 8, 8)
        out["single_digit"] = render("7", f16, 8, 8)
        big = "Größe"
        small = "Datei Bearbeiten Ansicht"
    else:
        out["dark_white"] = render("Merge visible layers", f16, 8, 6, (255, 255, 255), (30, 30, 30))
        out["transparent"] = render_transparent("Resize canvas", f16, 6)
        out["single_char"] = render("A", f16, 8, 8)
        out["single_digit"] = render("3", f16, 8, 8)
        out["outside_charset"] = render("Straße kostet 5 €", f16, 6, 6)
        big = "Pincel"
        small = "File Edit View Image"
    out.update(ui_crops(lang))
    # A line 400 px high and one 8 px high.
    font = ImageFont.truetype(serif, 300)
    im = render(big, font, 10, 10)
    canvas = Image.new("RGB", (im.width, 400), (255, 255, 255))
    canvas.paste(im, (0, (400 - im.height) // 2))
    out["tall_400"] = canvas
    font = ImageFont.truetype(dejavu, 7)
    im = render(small, font, 1, 1)
    canvas = Image.new("RGB", (im.width, 8), (255, 255, 255))
    canvas.paste(im, (0, (8 - im.height) // 2))
    out["short_8"] = canvas
    # A line 20 000 px wide: one corpus line repeated.
    unit = out[f"noto_x10_0"]
    reps = -(-20000 // unit.width)
    wide = Image.new("RGB", (unit.width * reps, unit.height), (255, 255, 255))
    for r in range(reps):
        wide.paste(unit, (r * unit.width, 0))
    out["wide_20000"] = wide
    return out


def main():
    version = subprocess.run([TESSERACT, "--version"], check=True, capture_output=True, text=True)
    with tempfile.TemporaryDirectory() as tmp:
        tessdata = Path(tmp) / "tessdata"
        (tessdata / "configs").mkdir(parents=True)
        (tessdata / "configs" / "tsv").write_text("tessedit_create_tsv 1\n")
        for lang in TEXTS:
            shutil.copy(CRATE / "models" / f"{lang}.traineddata", tessdata)
        env = dict(os.environ, TESSDATA_PREFIX=str(tessdata))
        for lang in TEXTS:
            target = HERE / lang
            if target.exists():
                shutil.rmtree(target)
            target.mkdir()
            for name, im in lines(lang).items():
                png = target / f"{name}.png"
                im.save(png)
                ref = Path(tmp) / f"{lang}_{name}.png"
                flatten(im).save(ref)
                tsv = subprocess.run(
                    [TESSERACT, str(ref), "-", "-l", lang, *ARGS],
                    check=True,
                    capture_output=True,
                    text=True,
                    env=env,
                ).stdout
                (target / f"{name}.tsv").write_text(tsv)
                print(lang, name, im.size, file=sys.stderr)
    (HERE / "reference.txt").write_text(
        "Reference: "
        + " ".join(["tesseract", "<flattened line.png>", "-", "-l", "<lang>", *ARGS])
        + "\n\n"
        + version.stdout
        + version.stderr
    )


if __name__ == "__main__":
    main()
