//! Reads path data back into polygons, so tests can check which pixel
//! centres an outline covers (even-odd rule).

/// One step of path data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Move(f64, f64),
    Line(f64, f64),
    Cubic([f64; 6]),
    Close,
}

/// Parses path data written as `M x y`, `l dx dy`, `c …` and `z`.
pub fn parse(d: &str) -> Vec<Step> {
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, toks: &mut Vec<String>| {
        if !cur.is_empty() {
            toks.push(std::mem::take(cur));
        }
    };
    for ch in d.chars() {
        match ch {
            'M' | 'l' | 'c' | 'z' => {
                flush(&mut cur, &mut toks);
                toks.push(ch.to_string());
            }
            ' ' => flush(&mut cur, &mut toks),
            '-' => {
                flush(&mut cur, &mut toks);
                cur.push(ch);
            }
            '.' if cur.contains('.') => {
                flush(&mut cur, &mut toks);
                cur.push(ch);
            }
            _ => cur.push(ch),
        }
    }
    flush(&mut cur, &mut toks);
    let num = |t: &String| {
        t.parse::<f64>()
            .unwrap_or_else(|_| panic!("bad number {t:?} in {d}"))
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let cmd = toks[i].clone();
        i += 1;
        match cmd.as_str() {
            "M" => {
                out.push(Step::Move(num(&toks[i]), num(&toks[i + 1])));
                i += 2;
            }
            "l" => {
                out.push(Step::Line(num(&toks[i]), num(&toks[i + 1])));
                i += 2;
            }
            "c" => {
                let mut v = [0.0; 6];
                for (k, slot) in v.iter_mut().enumerate() {
                    *slot = num(&toks[i + k]);
                }
                out.push(Step::Cubic(v));
                i += 6;
            }
            "z" => out.push(Step::Close),
            other => panic!("unexpected token {other:?} in {d}"),
        }
    }
    out
}

/// The outlines of path data as closed polygons (curves flattened).
pub fn polygons(d: &str) -> Vec<Vec<(f64, f64)>> {
    let mut polys = Vec::new();
    let mut poly: Vec<(f64, f64)> = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    for step in parse(d) {
        match step {
            Step::Move(mx, my) => {
                x = mx;
                y = my;
                poly = vec![(x, y)];
            }
            Step::Line(dx, dy) => {
                x += dx;
                y += dy;
                poly.push((x, y));
            }
            Step::Cubic([a, b, c, e, f, g]) => {
                let p0 = (x, y);
                let p1 = (x + a, y + b);
                let p2 = (x + c, y + e);
                let p3 = (x + f, y + g);
                for k in 1..=32 {
                    let t = f64::from(k) / 32.0;
                    let s = 1.0 - t;
                    poly.push((
                        s * s * s * p0.0
                            + 3.0 * s * s * t * p1.0
                            + 3.0 * s * t * t * p2.0
                            + t * t * t * p3.0,
                        s * s * s * p0.1
                            + 3.0 * s * s * t * p1.1
                            + 3.0 * s * t * t * p2.1
                            + t * t * t * p3.1,
                    ));
                }
                x = p3.0;
                y = p3.1;
            }
            Step::Close => polys.push(std::mem::take(&mut poly)),
        }
    }
    polys
}

/// Whether `(px, py)` lies inside the outlines by the even-odd rule.
pub fn contains(polys: &[Vec<(f64, f64)>], px: f64, py: f64) -> bool {
    let mut inside = false;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let (x1, y1) = poly[i];
            let (x2, y2) = poly[(i + 1) % n];
            if (y1 > py) != (y2 > py) {
                let xc = x1 + (py - y1) / (y2 - y1) * (x2 - x1);
                if px < xc {
                    inside = !inside;
                }
            }
        }
    }
    inside
}
