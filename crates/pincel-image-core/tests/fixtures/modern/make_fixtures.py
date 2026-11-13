"""Writes the JPEG XL fixtures (and one AVIF file) of docs/specs/image-formats-more-modern.md
and their reference PNGs, rendered by the reference library (libjxl) through `imagecodecs`:

    python -m venv venv && venv/bin/pip install imagecodecs numpy
    venv/bin/python make_fixtures.py

The reference PNG of a lossy file is libjxl's own decode of
that file, down-converted to 8 bit, so a decoder is checked against the
reference decoder, not against the source pixels.
"""

import io
import os

import imagecodecs
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))


def save(name, data):
    with open(os.path.join(HERE, name), 'wb') as f:
        f.write(data)


def png(name, pixels):
    """`pixels` as an 8-bit RGBA PNG."""
    rgba = to_rgba8(pixels)
    save(name, imagecodecs.png_encode(rgba))


def to_rgba8(pixels):
    p = np.asarray(pixels)
    if p.dtype == np.uint16:
        p = (p.astype(np.float64) / 65535.0 * 255.0 + 0.5).astype(np.uint8)
    elif p.dtype != np.uint8:
        raise TypeError(p.dtype)
    if p.ndim == 2:
        p = p[:, :, None]
    c = p.shape[2]
    if c == 1:
        p = np.concatenate([p, p, p, np.full_like(p, 255)], axis=2)
    elif c == 2:
        p = np.concatenate([p[:, :, :1]] * 3 + [p[:, :, 1:]], axis=2)
    elif c == 3:
        p = np.concatenate([p, np.full_like(p[:, :, :1], 255)], axis=2)
    return np.ascontiguousarray(p)


def scene(w, h, alpha=True, depth=8):
    """A smooth gradient with a few hard edges and transparent corners."""
    y, x = np.mgrid[0:h, 0:w]
    top = 255 if depth == 8 else 65535
    r = x * top // max(w - 1, 1)
    g = y * top // max(h - 1, 1)
    b = ((x // 4 + y // 4) % 2) * (top // 2) + top // 4
    chans = [r, g, b]
    if alpha:
        a = np.full_like(r, top)
        a[: h // 4, : w // 4] = 0
        a[h - h // 4 :, w - w // 4 :] = top // 3
        chans.append(a)
    dtype = np.uint8 if depth == 8 else np.uint16
    return np.stack(chans, axis=2).astype(dtype)


# JPEG XL -----------------------------------------------------------------

lossless8 = scene(24, 16)
data = imagecodecs.jpegxl_encode(lossless8, lossless=True, effort=7)
save('lossless8.jxl', data)
png('lossless8.png', lossless8)

lossy = scene(40, 24)
data = imagecodecs.jpegxl_encode(lossy, distance=1.0, effort=7)
save('lossy_alpha.jxl', data)
png('lossy_alpha.png', imagecodecs.jpegxl_decode(data))

lossy_rgb = scene(40, 24, alpha=False)
data = imagecodecs.jpegxl_encode(lossy_rgb, distance=1.5, effort=7)
save('lossy_rgb.jxl', data)
png('lossy_rgb.png', imagecodecs.jpegxl_decode(data))

deep = scene(20, 12, depth=16)
data = imagecodecs.jpegxl_encode(deep, lossless=True, effort=7)
save('lossless16.jxl', data)
png('lossless16.png', imagecodecs.jpegxl_decode(data))

data = imagecodecs.jpegxl_encode(lossless8, lossless=True, usecontainer=True)
save('container.jxl', data)

frames = np.stack([scene(16, 8), scene(16, 8)[::-1, ::-1]])
data = imagecodecs.jpegxl_encode(frames, lossless=True)
save('animated.jxl', data)
png('animated_first.png', frames[0])

full = imagecodecs.jpegxl_encode(lossy, distance=1.0)
save('truncated.jxl', full[: len(full) // 2])

# A header that declares 40000 x 10 px, with the pixel data cut off.
wide = imagecodecs.jpegxl_encode(np.zeros((10, 40000, 3), np.uint8), distance=1.0)
save('huge.jxl', wide[:64])

# AVIF --------------------------------------------------------------------

# Image mode does not decode AVIF (the spec's spike failed): one file that
# must fail to open (AC-04).
save('still.avif', imagecodecs.avif_encode(scene(32, 20), level=70))

print('ok')
