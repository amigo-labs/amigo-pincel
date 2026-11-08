//! Activations passed between layers: Tesseract's `NetworkIO`
//! (`src/lstm/networkio.cpp`) for a batch of one image, with its
//! `StrideMap` reduced to a height and a width.
//!
//! Time step `t` is pixel `(x, y)` at `t = y * width + x`. Integer
//! activations hold `value * 127` as `i8`; the softmax outputs are floats.

use crate::functions::{TRand, round_f32, round_f64};

/// The 2-D shape of an activation map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Map {
    pub(crate) height: usize,
    pub(crate) width: usize,
}

impl Map {
    pub(crate) fn steps(&self) -> usize {
        self.height * self.width
    }
}

/// Integer or float activations over a [`Map`].
#[derive(Debug, Clone)]
pub(crate) enum Data {
    Int(Vec<i8>),
    Float(Vec<f32>),
}

#[derive(Debug, Clone)]
pub(crate) struct NetworkIo {
    pub(crate) map: Map,
    pub(crate) features: usize,
    pub(crate) data: Data,
}

impl NetworkIo {
    pub(crate) fn new_int(map: Map, features: usize) -> Self {
        Self {
            map,
            features,
            data: Data::Int(vec![0; map.steps() * features]),
        }
    }

    pub(crate) fn new_float(map: Map, features: usize) -> Self {
        Self {
            map,
            features,
            data: Data::Float(vec![0.0; map.steps() * features]),
        }
    }

    /// A new map of the same kind (int or float) as `self`.
    pub(crate) fn like(&self, map: Map, features: usize) -> Self {
        match self.data {
            Data::Int(_) => Self::new_int(map, features),
            Data::Float(_) => Self::new_float(map, features),
        }
    }

    pub(crate) fn steps(&self) -> usize {
        self.map.steps()
    }

    pub(crate) fn int_row(&self, t: usize) -> &[i8] {
        match &self.data {
            Data::Int(v) => &v[t * self.features..(t + 1) * self.features],
            Data::Float(_) => &[],
        }
    }

    pub(crate) fn float_row(&self, t: usize) -> &[f32] {
        match &self.data {
            Data::Float(v) => &v[t * self.features..(t + 1) * self.features],
            Data::Int(_) => &[],
        }
    }

    /// Stores one int8 value (feature 0 of step `t`).
    pub(crate) fn write_raw(&mut self, t: usize, value: i8) {
        if let Data::Int(v) = &mut self.data {
            v[t * self.features] = value;
        }
    }

    /// `WriteTimeStepPart`: stores floats, rounding to `i8` in int mode.
    pub(crate) fn write_part(&mut self, t: usize, offset: usize, values: &[f32]) {
        let start = t * self.features + offset;
        match &mut self.data {
            Data::Int(v) => {
                for (d, &x) in v[start..start + values.len()].iter_mut().zip(values) {
                    *d = round_f32(x * 127.0).clamp(-127, 127) as i8;
                }
            }
            Data::Float(v) => v[start..start + values.len()].copy_from_slice(values),
        }
    }

    /// `CopyTimeStepGeneral`.
    pub(crate) fn copy_part(
        &mut self,
        dest_t: usize,
        dest_offset: usize,
        n: usize,
        src: &NetworkIo,
        src_t: usize,
        src_offset: usize,
    ) {
        let d = dest_t * self.features + dest_offset;
        let s = src_t * src.features + src_offset;
        match (&mut self.data, &src.data) {
            (Data::Int(dv), Data::Int(sv)) => dv[d..d + n].copy_from_slice(&sv[s..s + n]),
            (Data::Float(dv), Data::Float(sv)) => dv[d..d + n].copy_from_slice(&sv[s..s + n]),
            _ => {}
        }
    }

    /// `CopyTimeStepFrom`.
    pub(crate) fn copy_step(&mut self, dest_t: usize, src: &NetworkIo, src_t: usize) {
        let n = self.features;
        self.copy_part(dest_t, 0, n, src, src_t, 0);
    }

    /// `Randomize` in int mode: Tesseract pads with seeded random values.
    pub(crate) fn randomize(&mut self, t: usize, offset: usize, n: usize, rand: &mut TRand) {
        let start = t * self.features + offset;
        match &mut self.data {
            Data::Int(v) => {
                for d in &mut v[start..start + n] {
                    *d = round_f64(rand.signed_rand(127.0)) as i8;
                }
            }
            Data::Float(v) => {
                for d in &mut v[start..start + n] {
                    *d = rand.signed_rand(1.0) as f32;
                }
            }
        }
    }

    /// `MaxpoolTimeStep`.
    pub(crate) fn max_step(&mut self, dest_t: usize, src: &NetworkIo, src_t: usize) {
        let n = self.features;
        let d = dest_t * n;
        let s = src_t * src.features;
        match (&mut self.data, &src.data) {
            (Data::Int(dv), Data::Int(sv)) => {
                for (a, &b) in dv[d..d + n].iter_mut().zip(&sv[s..s + n]) {
                    if *a < b {
                        *a = b;
                    }
                }
            }
            (Data::Float(dv), Data::Float(sv)) => {
                for (a, &b) in dv[d..d + n].iter_mut().zip(&sv[s..s + n]) {
                    if *a < b {
                        *a = b;
                    }
                }
            }
            _ => {}
        }
    }

    /// `CopyWithXReversal`.
    pub(crate) fn x_reversed(&self) -> Self {
        let mut out = self.like(self.map, self.features);
        let w = self.map.width;
        for y in 0..self.map.height {
            for x in 0..w {
                out.copy_step(y * w + (w - 1 - x), self, y * w + x);
            }
        }
        out
    }

    /// `CopyWithYReversal`.
    pub(crate) fn y_reversed(&self) -> Self {
        let mut out = self.like(self.map, self.features);
        let (h, w) = (self.map.height, self.map.width);
        for y in 0..h {
            for x in 0..w {
                out.copy_step((h - 1 - y) * w + x, self, y * w + x);
            }
        }
        out
    }

    /// `CopyWithXYTranspose`.
    pub(crate) fn xy_transposed(&self) -> Self {
        let (h, w) = (self.map.height, self.map.width);
        let map = Map {
            height: w,
            width: h,
        };
        let mut out = self.like(map, self.features);
        for y in 0..h {
            for x in 0..w {
                out.copy_step(x * h + y, self, y * w + x);
            }
        }
        out
    }

    /// `CopyPacking` into a wider map.
    pub(crate) fn pack_into(&self, dest: &mut NetworkIo, offset: usize) {
        for t in 0..self.steps().min(dest.steps()) {
            dest.copy_part(t, offset, self.features, self, t, 0);
        }
    }
}
