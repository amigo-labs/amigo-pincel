#!/usr/bin/env python3
"""Regenerates the page corpus of pincel-ocr (docs/specs/ocr-page.md).

Renders every corpus page and writes, per language:

    tests/pages/<lang>/<name>.png        the page image the tests read
    tests/pages/<lang>/<name>.txt        the reference text: lines joined by a
                                         line break, blocks by an empty line
    tests/pages/<lang>/<name>.words.tsv  only for the rotated page: each word
                                         and its box in the rotated image

Pages named `ui_*` form the screenshot group, `print_*` the print group.
The page-text snapshots (`<name>.snapshot.txt`) are not written here: the
native test writes them with PINCEL_OCR_SNAPSHOTS=update.

The reference text follows the spec's definitions: every text item the
script draws is one line, and the blocks and their reading order come from
the definitions of text block and reading order applied to the items'
ink boxes (`blocks_in_order` below). Every reference text was checked by
hand against its image.

Needs Python 3 with Pillow (FreeType). The fonts (DejaVu 2.37 and
Liberation 2.1.5, both freely licensed) are downloaded once into a cache
directory (PINCEL_OCR_FONT_CACHE, default: the system temp directory) and
checked against pinned SHA-256 sums.

    python3 crates/pincel-ocr/tests/pages/make_pages.py
"""

import hashlib
import io
import math
import os
import random
import statistics
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent

FONT_ARCHIVES = {
    "dejavu": (
        "https://github.com/dejavu-fonts/dejavu-fonts/releases/download/version_2_37/dejavu-fonts-ttf-2.37.zip",
        "7576310b219e04159d35ff61dd4a4ec4cdba4f35c00e002a136f00e96a908b0a",
        {
            "DejaVuSans.ttf": "dejavu-fonts-ttf-2.37/ttf/DejaVuSans.ttf",
            "DejaVuSans-Bold.ttf": "dejavu-fonts-ttf-2.37/ttf/DejaVuSans-Bold.ttf",
        },
    ),
    "liberation": (
        "https://github.com/liberationfonts/liberation-fonts/files/7261482/liberation-fonts-ttf-2.1.5.tar.gz",
        "7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0",
        {
            "LiberationSans-Regular.ttf": "liberation-fonts-ttf-2.1.5/LiberationSans-Regular.ttf",
            "LiberationSans-Bold.ttf": "liberation-fonts-ttf-2.1.5/LiberationSans-Bold.ttf",
            "LiberationSerif-Regular.ttf": "liberation-fonts-ttf-2.1.5/LiberationSerif-Regular.ttf",
            "LiberationSerif-Bold.ttf": "liberation-fonts-ttf-2.1.5/LiberationSerif-Bold.ttf",
        },
    ),
}


def font_dir():
    cache = Path(os.environ.get("PINCEL_OCR_FONT_CACHE", Path(tempfile.gettempdir()) / "pincel-ocr-fonts"))
    cache.mkdir(parents=True, exist_ok=True)
    for url, sha, members in FONT_ARCHIVES.values():
        if all((cache / name).exists() for name in members):
            continue
        data = urllib.request.urlopen(url).read()
        if hashlib.sha256(data).hexdigest() != sha:
            raise SystemExit(f"checksum mismatch for {url}")
        if url.endswith(".zip"):
            archive = zipfile.ZipFile(io.BytesIO(data))
            read = archive.read
        else:
            archive = tarfile.open(fileobj=io.BytesIO(data))
            read = lambda m: archive.extractfile(m).read()  # noqa: E731
        for name, member in members.items():
            (cache / name).write_bytes(read(member))
    return cache


FONTS = font_dir()
_font_cache = {}


def font(name, size):
    key = (name, size)
    if key not in _font_cache:
        _font_cache[key] = ImageFont.truetype(str(FONTS / name), size)
    return _font_cache[key]


def x_height(f):
    left, top, right, bottom = f.getbbox("x", anchor="ls")
    return bottom - top


SANS = "LiberationSans-Regular.ttf"
SANS_BOLD = "LiberationSans-Bold.ttf"
SERIF = "LiberationSerif-Regular.ttf"
SERIF_BOLD = "LiberationSerif-Bold.ttf"
DEJAVU = "DejaVuSans.ttf"
DEJAVU_BOLD = "DejaVuSans-Bold.ttf"


class Page:
    """An image plus the text items drawn on it; each item is one line."""

    def __init__(self, width, height, mode="RGB", background=(255, 255, 255)):
        self.image = Image.new(mode, (width, height), background)
        self.draw = ImageDraw.Draw(self.image)
        self.items = []

    def rect(self, box, fill=None, outline=None, radius=0, width=1):
        self.draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)

    def text(self, x, baseline, text, f, fill, underline=(), strike=()):
        """Draws `text` with its left edge at x on `baseline`; `underline`
        and `strike` list the indexes of words to decorate."""
        self.draw.text((x, baseline), text, font=f, fill=fill, anchor="ls")
        box = self.draw.textbbox((x, baseline), text, font=f, anchor="ls")
        words = []
        pos = 0
        for i, word in enumerate(text.split(" ")):
            start = text.index(word, pos)
            pos = start + len(word)
            wx = x + f.getlength(text[:start])
            wbox = self.draw.textbbox((wx, baseline), word, font=f, anchor="ls")
            words.append((word, wbox))
            thick = max(1, round(f.size / 16))
            if i in underline:
                y = baseline + max(2, round(f.size * 0.12))
                self.draw.rectangle((wbox[0], y, wbox[2], y + thick - 1), fill=fill)
            if i in strike:
                y = baseline - round(x_height(f) * 0.5)
                self.draw.rectangle((wbox[0], y, wbox[2], y + thick - 1), fill=fill)
        self.items.append(
            {"text": " ".join(text.split()), "box": (box[0], box[1], box[2] - 1, box[3] - 1), "xh": x_height(f), "words": words}
        )
        return box


# ---------------------------------------------------------------------------
# Reading order, after the spec's definitions (ocr-page "Text block",
# "Column gap", "Reading order").


def overlap_x(a, b):
    return a[0] <= b[2] and b[0] <= a[2]


def overlap_y(a, b):
    return min(a[3], b[3]) - max(a[1], b[1]) + 1


def height(b):
    return b[3] - b[1] + 1


WARNINGS = []


def same_row(a, b):
    return overlap_y(a, b) >= 0.5 * min(height(a), height(b))


def line_height(line):
    """The line height of the block rule: the box height, but at least two
    x-heights, so a line without ascenders and descenders joins its
    paragraph (ocr-page Decisions)."""
    return max(height(line["box"]), 2 * line["xh"])


def blocks_in_order(items):
    lines = [dict(it) for it in items]
    n = len(lines)

    def neighbours(i, down):
        """The lines directly below (or above) line i: the nearest line
        that overlaps it horizontally, and the lines on that line's row."""
        a = lines[i]["box"]
        sign = 1 if down else -1
        out = [
            j
            for j in range(n)
            if j != i
            and overlap_x(a, lines[j]["box"])
            and not same_row(a, lines[j]["box"])
            and sign * (lines[j]["box"][1] + lines[j]["box"][3] - a[1] - a[3]) > 0
        ]
        if not out:
            return []
        near = min(out, key=lambda j: sign * (lines[j]["box"][1] if down else lines[j]["box"][3]))
        return [j for j in out if same_row(lines[near]["box"], lines[j]["box"])]

    # Lines on one row must be at least two x-heights apart, else they
    # would be one line.
    for i in range(n):
        for j in range(n):
            a, b = lines[i]["box"], lines[j]["box"]
            if i != j and same_row(a, b) and a[2] < b[0]:
                gap = b[0] - a[2] - 1
                xh = (lines[i]["xh"] + lines[j]["xh"]) / 2
                if gap < 2.8 * xh:
                    WARNINGS.append(f"row gap {gap} vs 2 xh {2 * xh}: {lines[i]['text']!r} / {lines[j]['text']!r}")
    nxt = {}
    for i in range(n):
        b = neighbours(i, True)
        if len(b) == 1 and neighbours(b[0], False) == [i]:
            a, c = lines[i], lines[b[0]]
            gap = c["box"][1] - a["box"][3] - 1
            limit = max(line_height(a), line_height(c))
            if 0.75 * limit < gap < 1.5 * limit:
                WARNINGS.append(f"block gap {gap} vs {limit}: {a['text']!r} / {c['text']!r}")
            if gap <= limit:
                nxt[i] = b[0]
    has_prev = set(nxt.values())
    blocks = []
    for i in range(n):
        if i in has_prev:
            continue
        chain = [i]
        while chain[-1] in nxt:
            chain.append(nxt[chain[-1]])
        boxes = [lines[k]["box"] for k in chain]
        blocks.append(
            {
                "lines": [lines[k] for k in chain],
                "box": (
                    min(b[0] for b in boxes),
                    min(b[1] for b in boxes),
                    max(b[2] for b in boxes),
                    max(b[3] for b in boxes),
                ),
            }
        )
    return order(blocks)


def gaps(intervals):
    """Gaps between the union of closed integer intervals: (start, end)."""
    out = []
    intervals = sorted(intervals)
    reach = intervals[0][1]
    for lo, hi in intervals[1:]:
        if lo > reach + 1:
            out.append((reach + 1, lo - 1))
        reach = max(reach, hi)
    return out


def median_xh(blocks):
    return statistics.median(line["xh"] for b in blocks for line in b["lines"])


def order(blocks, check_ties=True):
    if len(blocks) <= 1:
        return blocks
    for lo, hi in gaps([(b["box"][0], b["box"][2]) for b in blocks]):
        left = [b for b in blocks if b["box"][2] < lo]
        right = [b for b in blocks if b["box"][0] > hi]
        limit = 2 * median_xh(left + right)
        if 0.7 * limit < hi - lo + 1 < 1.4 * limit:
            WARNINGS.append(
                f"column gap {hi - lo + 1} vs {limit} at x {lo}: "
                f"{[b['lines'][0]['text'] for b in left]} | {[b['lines'][0]['text'] for b in right]}"
            )
        if hi - lo + 1 >= limit:
            return order(left, check_ties) + order(right, check_ties)
    hgaps = gaps([(b["box"][1], b["box"][3]) for b in blocks])
    if hgaps:
        def cut(g, check):
            top = [b for b in blocks if b["box"][3] < g[0]]
            bottom = [b for b in blocks if b["box"][1] > g[1]]
            return order(top, check) + order(bottom, check)

        widest = max(hgaps, key=lambda g: (g[1] - g[0], -g[0]))
        result = cut(widest, check_ties)
        if check_ties:
            names = [id(b) for b in result]
            for g in hgaps:
                if g != widest and (widest[1] - widest[0]) - (g[1] - g[0]) < 4:
                    if [id(b) for b in cut(g, False)] != names:
                        WARNINGS.append(f"row gaps at y {widest[0]} and {g[0]} nearly tie and give other orders")
        return result
    return sorted(blocks, key=lambda b: (b["box"][1], b["box"][0]))


def reference(items):
    return "\n\n".join("\n".join(line["text"] for line in b["lines"]) for b in blocks_in_order(items)) + "\n"


# ---------------------------------------------------------------------------
# Texts.

T = {
    "eng": {
        "menu": ["File", "Edit", "View", "Image", "Layer", "Select", "Filter", "Help"],
        "nav_head": "PROJECT",
        "nav": ["Overview", "Layers", "Brushes", "Colors", "History", "Export", "Settings"],
        "nav2_head": "RECENT FILES",
        "nav2": ["sunset_sprites.png", "title_screen.png", "tiles_forest.png", "hero_walk_cycle.png", "ui_buttons_v2.png"],
        "about": "About this project",
        "about_text": [
            "Created on 12 March 2026 by Maria Lopez. Last opened yesterday at 18:40.",
            "The project folder contains 46 files with a total size of 128 MB.",
        ],
        "title": "Project settings",
        "intro": [
            "These settings apply to every image in this project. Changes are saved",
            "automatically and can be undone from the History panel at any time.",
        ],
        "form": [("Name", "Sunset sprite sheet"), ("Author", "Maria Lopez"), ("Size", "1920 x 1080 px"), ("Format", "PNG with alpha")],
        "buttons": ["Save changes", "Cancel"],
        "sect": "Export options",
        "sect_text": [
            "Export the current image as PNG, JPEG or WebP. The quality setting only",
            "affects JPEG and WebP files; PNG files are always lossless.",
            "Large images may take a few seconds to export.",
        ],
        "panel_head": "Properties",
        "panel": ["Width: 1920 px", "Height: 1080 px", "Resolution: 72 dpi", "Layers: 12", "Opacity: 85%", "Blend mode: Normal"],
        "panel2_head": "Notes",
        "panel2": ["Check the colors on a", "second monitor before", "the final export."],
        "status_left": "Ready",
        "status_right": "Zoom 100%",
        # dark UI
        "d_title": "Inbox",
        "d_folders": ["Inbox (12)", "Starred", "Sent", "Drafts (3)", "Archive", "Spam"],
        "d_mails": [
            ("Anna Schmidt", "Meeting notes for Thursday", "10:42"),
            ("Build server", "Nightly build passed: 214 tests", "09:15"),
            ("Tom Becker", "Re: Sprite sheet for level 4", "Yesterday"),
            ("Design team", "New icons are ready for review", "Yesterday"),
            ("Calendar", "Reminder: release planning at 3 pm", "Mon"),
        ],
        "d_subject": "Meeting notes for Thursday",
        "d_body": [
            "Hi everyone,",
            "here are the notes from our last meeting. We agreed to finish the",
            "animation export before the end of the month and to test the new",
            "brush engine on large images. Please send me your comments by Friday.",
            "Best regards, Anna",
        ],
        "d_buttons": ["Reply", "Forward"],
        # mockup
        "m_brand": "PixelShop",
        "m_nav": ["Home", "Products", "Pricing", "Contact"],
        "m_menu": ["New project", "Open file", "Import images", "Recent projects"],
        "m_title": "Welcome back, Sam",
        "m_text": ["You have three projects waiting for review.", "Pick one to continue where you left off."],
        "m_buttons": ["Save", "Delete", "Share"],
        "m_card_title": "Storage almost full",
        "m_card": ["You are using 9.2 GB of 10 GB.", "Upgrade your plan to get more space."],
        "m_banner": "New: export animations as GIF and WebP",
        # transparent
        "t_lines": ["Level complete", "Score: 12,480 points", "Time: 3 min 25 s", "Press any key to continue"],
        # print
        "p_title": "The History of Printing",
        "p_par1": (
            "Printing with movable type changed the way people shared knowledge. "
            "Before the printing press, books were copied by hand, which took months "
            "and made every copy expensive. Within a few decades, printed books became "
            "common in many European cities, and the number of readers grew quickly."
        ),
        "p_par2": (
            "Early printers mixed their own ink from soot, varnish and egg white. They "
            "cast letters from an alloy of lead, tin and antimony, which melted at a low "
            "temperature and did not shrink when it cooled. A skilled team could print "
            "several hundred sheets in a single day."
        ),
        "p_under": "movable",
        "p_strike": "months",
        "c_title": "Notes on Colour and Light",
        "c_par": [
            (
                "Every colour we see is light that an object reflects. A red apple absorbs "
                "most of the light that falls on it and reflects mainly the red part of the "
                "spectrum back to our eyes."
            ),
            (
                "Painters mix pigments, which subtract light, while screens add light. "
                "That is why mixing paints gives darker colours, but mixing coloured light "
                "on a screen gives brighter ones."
            ),
            (
                "Printers use cyan, magenta, yellow and black inks. Screens use red, green "
                "and blue pixels. Converting an image between the two systems always changes "
                "some of its colours."
            ),
            (
                "A good monitor should be calibrated at least once a year, so that the colours "
                "on the screen match the colours of the printed result."
            ),
        ],
        "s_head": "Pages",
        "s_body": [
            "This page mixes text of very different sizes. The heading above is",
            "large enough to read from across the room, while the note at the",
            "bottom of the page is tiny and needs a magnifying glass.",
        ],
        "s_foot": "1) Small print: all sizes are given in pixels of the page image.",
    },
    "deu": {
        "menu": ["Datei", "Bearbeiten", "Ansicht", "Bild", "Ebene", "Auswahl", "Hilfe"],
        "nav_head": "PROJEKT",
        "nav": ["Übersicht", "Ebenen", "Pinsel", "Farben", "Verlauf", "Exportieren", "Einstellungen"],
        "nav2_head": "LETZTE DATEIEN",
        "nav2": ["sonnenuntergang.png", "titelbild.png", "kacheln_wald.png", "held_laufen.png", "knoepfe_v2.png"],
        "about": "Über dieses Projekt",
        "about_text": [
            "Erstellt am 12. März 2026 von Jürgen Weiß. Zuletzt geöffnet gestern um 18:40.",
            "Der Projektordner enthält 46 Dateien mit insgesamt 128 MB.",
        ],
        "title": "Projekteinstellungen",
        "intro": [
            "Diese Einstellungen gelten für alle Bilder in diesem Projekt. Änderungen",
            "werden automatisch gespeichert und lassen sich im Verlauf rückgängig machen.",
        ],
        "form": [("Name", "Sprites für den Abend"), ("Autorin", "Jürgen Weiß"), ("Größe", "1920 x 1080 Pixel"), ("Format", "PNG mit Alpha")],
        "buttons": ["Änderungen speichern", "Abbrechen"],
        "sect": "Exportoptionen",
        "sect_text": [
            "Das aktuelle Bild lässt sich als PNG, JPEG oder WebP exportieren. Die Qualität",
            "wirkt nur auf JPEG- und WebP-Dateien; PNG-Dateien sind immer verlustfrei.",
            "Große Bilder brauchen beim Export einige Sekunden.",
        ],
        "panel_head": "Eigenschaften",
        "panel": ["Breite: 1920 Pixel", "Höhe: 1080 Pixel", "Auflösung: 72 dpi", "Ebenen: 12", "Deckkraft: 85 %", "Modus: Normal"],
        "panel2_head": "Notizen",
        "panel2": ["Farben vor dem Export", "auf einem zweiten", "Bildschirm prüfen."],
        "status_left": "Bereit",
        "status_right": "Zoom 100 %",
        "d_title": "Posteingang",
        "d_folders": ["Posteingang (12)", "Markiert", "Gesendet", "Entwürfe (3)", "Archiv", "Spam"],
        "d_mails": [
            ("Anna Schmidt", "Protokoll für Donnerstag", "10:42"),
            ("Buildserver", "Nächtlicher Build: 214 Tests grün", "09:15"),
            ("Tom Becker", "Re: Sprites für Level 4", "Gestern"),
            ("Designteam", "Neue Symbole zur Durchsicht", "Gestern"),
            ("Kalender", "Erinnerung: Planung um 15 Uhr", "Mo."),
        ],
        "d_subject": "Protokoll für Donnerstag",
        "d_body": [
            "Hallo zusammen,",
            "hier ist das Protokoll unseres letzten Treffens. Wir möchten den",
            "Animationsexport bis Monatsende fertigstellen und den neuen Pinsel",
            "an großen Bildern testen. Bitte schickt mir eure Anmerkungen bis Freitag.",
            "Viele Grüße, Anna",
        ],
        "d_buttons": ["Antworten", "Weiterleiten"],
        "m_brand": "PixelShop",
        "m_nav": ["Start", "Produkte", "Preise", "Kontakt"],
        "m_menu": ["Neues Projekt", "Datei öffnen", "Bilder importieren", "Letzte Projekte"],
        "m_title": "Willkommen zurück, Sam",
        "m_text": ["Drei Projekte warten auf deine Durchsicht.", "Wähle eines, um dort weiterzumachen."],
        "m_buttons": ["Speichern", "Löschen", "Teilen"],
        "m_card_title": "Speicher fast voll",
        "m_card": ["Du nutzt 9,2 GB von 10 GB.", "Mit einem größeren Tarif hast du mehr Platz."],
        "m_banner": "Neu: Animationen als GIF und WebP exportieren",
        "t_lines": ["Level geschafft", "Punkte: 12.480", "Zeit: 3 min 25 s", "Weiter mit einer beliebigen Taste"],
        "p_title": "Die Geschichte des Buchdrucks",
        "p_par1": (
            "Der Druck mit beweglichen Lettern veränderte die Weitergabe von Wissen. "
            "Vor der Druckerpresse wurden Bücher von Hand abgeschrieben, was Monate "
            "dauerte und jede Abschrift teuer machte. Innerhalb weniger Jahrzehnte waren "
            "gedruckte Bücher in vielen Städten Europas verbreitet, und die Zahl der "
            "Leser wuchs schnell."
        ),
        "p_par2": (
            "Frühe Drucker mischten ihre Farbe selbst aus Ruß, Firnis und Eiweiß. Die "
            "Lettern gossen sie aus einer Legierung von Blei, Zinn und Antimon, die bei "
            "niedriger Temperatur schmolz und beim Abkühlen kaum schrumpfte. Eine geübte "
            "Werkstatt druckte mehrere hundert Bogen an einem Tag."
        ),
        "p_under": "beweglichen",
        "p_strike": "Monate",
        "c_title": "Über Farbe und Licht",
        "c_par": [
            (
                "Jede Farbe, die wir sehen, ist Licht, das ein Gegenstand zurückwirft. Ein "
                "roter Apfel schluckt den größten Teil des Lichts und wirft vor allem den "
                "roten Anteil des Spektrums zurück."
            ),
            (
                "Maler mischen Pigmente, die Licht abziehen, während Bildschirme Licht "
                "addieren. Deshalb ergeben gemischte Farben dunklere Töne, gemischtes Licht "
                "auf dem Bildschirm aber hellere."
            ),
            (
                "Druckereien verwenden Cyan, Magenta, Gelb und Schwarz. Bildschirme nutzen "
                "rote, grüne und blaue Pixel. Beim Umrechnen zwischen beiden Systemen ändern "
                "sich immer einige Farben."
            ),
            (
                "Ein guter Monitor sollte mindestens einmal im Jahr kalibriert werden, damit "
                "die Farben auf dem Bildschirm zum gedruckten Ergebnis passen."
            ),
        ],
        "s_head": "Seite",
        "s_body": [
            "Auf dieser Seite stehen Texte sehr verschiedener Größe. Die Überschrift",
            "oben ist quer durch den Raum lesbar, während die Fußnote am Ende",
            "der Seite winzig ist und eine Lupe braucht.",
        ],
        "s_foot": "1) Kleingedrucktes: alle Größen in Pixeln des Seitenbildes.",
    },
}


def wrap(text, f, width):
    lines, line = [], ""
    for word in text.split(" "):
        cand = word if not line else line + " " + word
        if f.getlength(cand) <= width:
            line = cand
        else:
            lines.append(line)
            line = word
    lines.append(line)
    return lines


# ---------------------------------------------------------------------------
# Screenshot group.


def icon(page, x, y, kind, color):
    if kind == "square":
        page.rect((x, y, x + 13, y + 13), outline=color, width=2)
    elif kind == "circle":
        page.draw.ellipse((x, y, x + 13, y + 13), fill=color)
    elif kind == "triangle":
        page.draw.polygon([(x, y + 13), (x + 7, y), (x + 14, y + 13)], fill=color)


def ui_light(t):
    page = Page(1920, 1080, background=(255, 255, 255))
    ink, grey = (32, 33, 36), (95, 99, 104)
    f, fb, fh, fs = font(SANS, 15), font(SANS_BOLD, 13), font(SANS_BOLD, 26), font(SANS_BOLD, 18)
    # Menu bar.
    page.rect((0, 0, 1919, 33), fill=(243, 243, 243))
    page.text(14, 22, "  ".join(t["menu"]), f, ink)
    # Sidebar.
    page.rect((0, 34, 279, 1047), fill=(247, 248, 250))
    page.rect((280, 34, 280, 1047), fill=(218, 220, 224))
    page.text(24, 80, t["nav_head"], fb, grey)
    kinds = ["square", "circle", "triangle", None, None, None, None]
    for i, item in enumerate(t["nav"]):
        y = 130 + i * 40
        if i == 1:
            page.rect((12, y - 24, 268, y + 12), fill=(225, 236, 254), radius=6)
        if kinds[i]:
            icon(page, 32, y - 13, kinds[i], (66, 133, 244))
        page.text(56, y, item, f, ink)
    page.text(24, 440, t["nav2_head"], fb, grey)
    for i, item in enumerate(t["nav2"]):
        page.text(24, 476 + i * 22, item, f, ink)
    # Main area.
    mx = 330
    page.text(mx, 100, t["title"], fh, ink)
    for i, line in enumerate(t["intro"]):
        page.text(mx, 132 + i * 20, line, f, grey)
    for i, (label, value) in enumerate(t["form"]):
        y = 250 + i * 52
        page.text(mx, y, label, f, ink)
        page.rect((mx + 170, y - 24, mx + 620, y + 10), outline=(160, 164, 170), radius=4)
        page.text(mx + 184, y, value, f, ink)
    by = 490
    w0 = f.getlength(t["buttons"][0]) + 40
    page.rect((mx, by, mx + w0, by + 36), fill=(26, 115, 232), radius=6)
    page.text(mx + 20, by + 23, t["buttons"][0], f, (255, 255, 255))
    w1 = f.getlength(t["buttons"][1]) + 40
    page.rect((mx + w0 + 16, by, mx + w0 + 16 + w1, by + 36), outline=(26, 115, 232), radius=6, width=1)
    page.text(mx + w0 + 36, by + 23, t["buttons"][1], f, (26, 115, 232))
    page.text(mx, 600, t["sect"], fs, ink)
    for i, line in enumerate(t["sect_text"]):
        page.text(mx, 626 + i * 20, line, f, grey)
    page.text(mx, 760, t["about"], fs, ink)
    for i, line in enumerate(t["about_text"]):
        page.text(mx, 786 + i * 20, line, f, grey)
    # Right panel.
    px = 1560
    page.rect((1530, 34, 1530, 1047), fill=(218, 220, 224))
    page.text(px, 100, t["panel_head"], fs, ink)
    for i, line in enumerate(t["panel"]):
        page.text(px, 156 + i * 22, line, f, ink)
    page.text(px, 400, t["panel2_head"], fs, ink)
    for i, line in enumerate(t["panel2"]):
        page.text(px, 452 + i * 20, line, f, grey)
    # Status bar.
    page.rect((0, 1048, 1919, 1079), fill=(232, 234, 237))
    page.text(14, 1069, t["status_left"], f, ink)
    page.text(1906 - f.getlength(t["status_right"]), 1069, t["status_right"], f, ink)
    return page


def ui_dark(t):
    page = Page(1600, 900, background=(30, 30, 30))
    ink, dim, accent = (212, 212, 212), (150, 150, 150), (110, 180, 255)
    f, fb, fh = font(DEJAVU, 14), font(DEJAVU_BOLD, 14), font(DEJAVU_BOLD, 22)
    page.rect((0, 0, 239, 899), fill=(37, 37, 38))
    page.text(24, 52, t["d_title"], fh, ink)
    for i, item in enumerate(t["d_folders"]):
        y = 110 + i * 40
        if i == 0:
            page.rect((10, y - 24, 229, y + 10), fill=(9, 71, 113), radius=4)
        page.text(28, y, item, f, ink)
    # Mail list.
    page.rect((240, 0, 760, 899), fill=(30, 30, 30))
    page.rect((760, 0, 760, 899), fill=(60, 60, 60))
    for i, (sender, subject, when) in enumerate(t["d_mails"]):
        y = 60 + i * 84
        page.text(268, y, sender, fb, ink)
        page.text(740 - f.getlength(when), y, when, f, dim)
        page.text(268, y + 22, subject, f, dim)
        page.rect((256, y + 46, 744, y + 46), fill=(55, 55, 55))
    # Reading pane.
    page.text(800, 60, t["d_subject"], fh, ink)
    for i, line in enumerate(t["d_body"]):
        page.text(800, 130 + i * 21, line, f, ink)
    by = 330
    w0 = f.getlength(t["d_buttons"][0]) + 36
    page.rect((800, by, 800 + w0, by + 34), fill=(14, 99, 156), radius=4)
    page.text(818, by + 22, t["d_buttons"][0], f, (255, 255, 255))
    w1 = f.getlength(t["d_buttons"][1]) + 36
    page.rect((816 + w0, by, 816 + w0 + w1, by + 34), outline=accent, radius=4)
    page.text(834 + w0, by + 22, t["d_buttons"][1], f, accent)
    return page


def ui_mockup(t):
    page = Page(1280, 800, background=(250, 250, 250))
    white, dark = (255, 255, 255), (33, 33, 33)
    f, fb, fh, fn = font(SANS, 16), font(SANS_BOLD, 16), font(SANS_BOLD, 28), font(SANS_BOLD, 20)
    # Navigation bar, light text on dark.
    page.rect((0, 0, 1279, 55), fill=(38, 50, 56))
    page.text(24, 36, t["m_brand"], fn, white)
    x = 300
    for item in t["m_nav"]:
        page.text(x, 34, item, f, (236, 239, 241))
        x += f.getlength(item) + 48
    # Opened menu below the first entry.
    page.rect((290, 60, 560, 220), fill=white, outline=(189, 189, 189))
    for i, item in enumerate(t["m_menu"]):
        page.text(308, 94 + i * 24, item, f, dark)
    # Welcome text.
    page.text(640, 120, t["m_title"], fh, dark)
    for i, line in enumerate(t["m_text"]):
        page.text(640, 156 + i * 24, line, f, (84, 84, 84))
    # Coloured buttons.
    x = 640
    for item, color in zip(t["m_buttons"], [(46, 125, 50), (198, 40, 40), (21, 101, 192)]):
        w = f.getlength(item) + 44
        page.rect((x, 250, x + w, 292), fill=color, radius=8)
        page.text(x + 22, 277, item, fb, white)
        x += w + 24
    # Dark card.
    page.rect((60, 380, 600, 560), fill=(55, 71, 79), radius=10)
    page.text(90, 430, t["m_card_title"], fn, white)
    for i, line in enumerate(t["m_card"]):
        page.text(90, 462 + i * 24, line, f, (207, 216, 220))
    # Yellow banner.
    page.rect((640, 380, 1220, 440), fill=(255, 235, 59), radius=6)
    page.text(664, 417, t["m_banner"], fb, dark)
    return page


def ui_transparent(t):
    page = Page(900, 400, mode="RGBA", background=(255, 255, 255, 0))
    fh, f = font(SANS_BOLD, 40), font(SANS, 24)
    page.text(40, 80, t["t_lines"][0], fh, (255, 255, 255, 255))
    for i, line in enumerate(t["t_lines"][1:]):
        page.text(40, 180 + i * 32, line, f, (255, 255, 255, 255))
    return page


# ---------------------------------------------------------------------------
# Print group (300 dpi: 10 pt is about 42 px).


def print_single(t, width=1800):
    page = Page(width, 1300, mode="L", background=255)
    fh, f = font(SERIF_BOLD, 58), font(SERIF, 42)
    page.text(150, 200, t["p_title"], fh, 0)
    y = 340
    for par in (t["p_par1"], t["p_par2"]):
        for line in wrap(par, f, width - 300):
            words = line.split(" ")
            under = [i for i, w in enumerate(words) if w.strip(".,") == t["p_under"]]
            strike = [i for i, w in enumerate(words) if w.strip(".,") == t["p_strike"]]
            page.text(150, y, line, f, 0, underline=under, strike=strike)
            y += 54
        y += 54
    return page


def print_columns(t):
    page = Page(1800, 1400, mode="L", background=255)
    fh, f = font(SERIF_BOLD, 62), font(SERIF, 40)
    page.text(150, 200, t["c_title"], fh, 0)
    col_w, gutter = 705, 90
    for c in range(2):
        x = 150 + c * (col_w + gutter)
        y = 340
        for par in t["c_par"][2 * c : 2 * c + 2]:
            for line in wrap(par, f, col_w):
                page.text(x, y, line, f, 0)
                y += 52
            y += 52
    return page


def rotate_point(x, y, w, h, nw, nh, deg):
    """Where PIL's `rotate(deg, expand=True)` moves the point (x, y)."""
    a = math.radians(deg)
    dx, dy = x - w / 2, y - h / 2
    return nw / 2 + dx * math.cos(a) + dy * math.sin(a), nh / 2 - dx * math.sin(a) + dy * math.cos(a)


def print_rotated(t):
    page = print_single(t)
    w, h = page.image.size
    rotated = page.image.rotate(3, resample=Image.BICUBIC, expand=True, fillcolor=255)
    nw, nh = rotated.size
    out = Page(nw, nh, mode="L", background=255)
    out.image = rotated
    words = []
    for item in page.items:
        for word, (l, top, r, b) in item["words"]:
            pts = [rotate_point(x, y, w, h, nw, nh, 3) for x, y in ((l, top), (r, top), (l, b), (r, b))]
            xs, ys = [p[0] for p in pts], [p[1] for p in pts]
            words.append((word, math.floor(min(xs)), math.floor(min(ys)), math.ceil(max(xs)) - 1, math.ceil(max(ys)) - 1))
    # Reading order of the rotated page is that of the upright one.
    out.items = page.items
    return out, words


def print_scan(t):
    page = print_single(t)
    rng = random.Random("pincel-ocr-scan")
    im = page.image.filter(ImageFilter.GaussianBlur(1.0))
    noise = Image.frombytes("L", im.size, bytes(rng.randrange(256) for _ in range(im.width * im.height)))
    # Gaussian-like noise from the sum of uniform samples, sd about 7 levels.
    px = im.load()
    npx = noise.load()
    for y in range(im.height):
        for x in range(im.width):
            v = px[x, y] - 12 + (npx[x, y] * 24) // 255 + rng.randint(-6, 6)
            px[x, y] = max(0, min(255, v))
    page.image = im
    return page


def print_sizes(t):
    page = Page(2200, 1300, mode="L", background=255)
    fh, f, ff = font(SANS, 566), font(SERIF, 42), font(SANS, 12)
    page.text(120, 560, t["s_head"], fh, 0)
    for i, line in enumerate(t["s_body"]):
        page.text(120, 800 + i * 54, line, f, 0)
    page.text(120, 1200, t["s_foot"], ff, 0)
    return page


LARGE_LINES = [
    "The large page holds three lines of text.",
    "This line sits in the middle of the page.",
    "And this one is close to the bottom edge.",
]


def large_lines():
    """Three single-line strips the large-page test (AC-12) pastes onto a
    white 8192 x 8192 px page."""
    target = HERE / "large"
    target.mkdir(exist_ok=True)
    f = font(SERIF, 42)
    for i, text in enumerate(LARGE_LINES, 1):
        left, top, right, bottom = f.getbbox(text, anchor="ls")
        im = Image.new("L", (right - left + 24, bottom - top + 24), 255)
        ImageDraw.Draw(im).text((12 - left, 12 - top), text, font=f, fill=0, anchor="ls")
        im.save(target / f"line_{i}.png", optimize=True)
    with open(target / "lines.txt", "w", encoding="utf-8", newline="") as out:
        out.write("\n\n".join(LARGE_LINES) + "\n")


def main():
    large_lines()
    for lang, t in T.items():
        target = HERE / lang
        target.mkdir(exist_ok=True)
        for old in target.glob("*"):
            if not old.name.endswith(".snapshot.txt"):
                old.unlink()
        pages = {
            "ui_light": ui_light(t),
            "ui_dark": ui_dark(t),
            "ui_mockup": ui_mockup(t),
            "ui_transparent": ui_transparent(t),
            "print_single": print_single(t),
            "print_columns": print_columns(t),
            "print_scan": print_scan(t),
            "print_sizes": print_sizes(t),
        }
        rotated, words = print_rotated(t)
        pages["print_rotated"] = rotated
        with open(target / "print_rotated.words.tsv", "w", encoding="utf-8", newline="") as out:
            out.write("word\tleft\ttop\tright\tbottom\n")
            for word, l, top, r, b in words:
                out.write(f"{word}\t{l}\t{top}\t{r}\t{b}\n")
        for name, page in pages.items():
            page.image.save(target / f"{name}.png", optimize=True)
            with open(target / f"{name}.txt", "w", encoding="utf-8", newline="") as out:
                out.write(reference(page.items))
            print(lang, name, page.image.size)
            for w in WARNINGS:
                print("  warning:", w)
            WARNINGS.clear()


if __name__ == "__main__":
    main()
