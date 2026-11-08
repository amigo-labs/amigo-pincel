//! The LSTM network: deserialization (`Network::CreateFromFile` and each
//! layer's `DeSerialize`) and the int-mode forward pass of the layer types
//! Tesseract's models use (`src/lstm/*.cpp`).

mod io;

pub(crate) use io::{Map, NetworkIo};

use crate::error::LoadError;
use crate::functions::{TRand, logistic, softmax_in_place, tanh};
use crate::reader::Reader;

/// Tesseract's `NetworkType`, in serialization order.
const TYPE_NAMES: [&str; 27] = [
    "Invalid",
    "Input",
    "Convolve",
    "Maxpool",
    "Parallel",
    "Replicated",
    "ParBidiLSTM",
    "DepParUDLSTM",
    "Par2dLSTM",
    "Series",
    "Reconfig",
    "RTLReversed",
    "TTBReversed",
    "XYTranspose",
    "LSTM",
    "SummLSTM",
    "Logistic",
    "LinLogistic",
    "LinTanh",
    "Tanh",
    "Relu",
    "Linear",
    "Softmax",
    "SoftmaxNoCTC",
    "LSTMSoftmax",
    "LSTMBinarySoftmax",
    "TensorFlow",
];

mod nt {
    pub(super) const INPUT: usize = 1;
    pub(super) const CONVOLVE: usize = 2;
    pub(super) const MAXPOOL: usize = 3;
    pub(super) const PARALLEL: usize = 4;
    pub(super) const REPLICATED: usize = 5;
    pub(super) const PAR_RL_LSTM: usize = 6;
    pub(super) const PAR_UD_LSTM: usize = 7;
    pub(super) const SERIES: usize = 9;
    pub(super) const RECONFIG: usize = 10;
    pub(super) const XREVERSED: usize = 11;
    pub(super) const YREVERSED: usize = 12;
    pub(super) const XYTRANSPOSE: usize = 13;
    pub(super) const LSTM: usize = 14;
    pub(super) const LSTM_SUMMARY: usize = 15;
    pub(super) const LOGISTIC: usize = 16;
    pub(super) const POSCLIP: usize = 17;
    pub(super) const SYMCLIP: usize = 18;
    pub(super) const TANH: usize = 19;
    pub(super) const RELU: usize = 20;
    pub(super) const LINEAR: usize = 21;
    pub(super) const SOFTMAX: usize = 22;
    pub(super) const SOFTMAX_NO_CTC: usize = 23;
    pub(super) const LSTM_SOFTMAX: usize = 24;
    pub(super) const LSTM_SOFTMAX_ENCODED: usize = 25;
}

/// `NF_LAYER_SPECIFIC_LR`: a plumbing layer stores per-layer learning rates.
const NF_LAYER_SPECIFIC_LR: i32 = 64;
/// Tesseract's `kStateClip`.
const STATE_CLIP: f32 = 100.0;
/// Deepest nesting of plumbing layers accepted, so a corrupt model cannot
/// overflow the stack.
const MAX_DEPTH: usize = 32;

/// An int8 weight matrix: `outputs` rows of `inputs + 1` weights (the last
/// one is the bias), with one scale per row (`WeightMatrix`).
#[derive(Debug, Clone)]
pub(crate) struct Weights {
    rows: usize,
    cols: usize,
    w: Vec<i8>,
    scales: Vec<f32>,
}

impl Weights {
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let mode = r.u8()?;
        const INT8: u8 = 1;
        const DOUBLE: u8 = 128;
        if mode & INT8 == 0 {
            return Err(LoadError::FloatWeights);
        }
        if mode & DOUBLE == 0 {
            // The old float-scale format, which Tesseract's FAST_FLOAT build
            // cannot load either.
            return Err(LoadError::NoLstm);
        }
        let rows = r.u32()? as usize;
        let cols = r.u32()? as usize;
        if rows == 0 || cols == 0 || rows > usize::from(u16::MAX) || cols > usize::from(u16::MAX) {
            return Err(LoadError::NoLstm);
        }
        let _empty = r.i8()?;
        let w: Vec<i8> = r.bytes(rows * cols)?.iter().map(|&b| b as i8).collect();
        let n = r.count(8)?;
        if n < rows {
            return Err(LoadError::NoLstm);
        }
        let mut scales = Vec::with_capacity(n);
        for _ in 0..n {
            scales.push((r.f64()? / 127.0) as f32);
        }
        Ok(Self {
            rows,
            cols,
            w,
            scales,
        })
    }

    fn inputs(&self) -> usize {
        self.cols - 1
    }

    /// `IntSimdMatrix::MatrixDotVector`, the generic path.
    fn dot(&self, u: &[i8], v: &mut [f32]) {
        let ni = self.inputs();
        for (i, out) in v.iter_mut().enumerate().take(self.rows) {
            let row = &self.w[i * self.cols..(i + 1) * self.cols];
            // At most 65 535 products of ±127²: the sum fits an i32, so
            // wrapping adds change nothing and let the loop vectorize.
            let total: i32 = row[..ni].iter().zip(&u[..ni]).fold(0i32, |acc, (&a, &b)| {
                acc.wrapping_add(i32::from(a).wrapping_mul(i32::from(b)))
            });
            *out = (total + i32::from(row[ni]) * 127) as f32 * self.scales[i];
        }
    }
}

/// The non-linearity of a fully connected layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activation {
    Logistic,
    PosClip,
    SymClip,
    Tanh,
    Relu,
    Linear,
    Softmax,
}

impl Activation {
    fn apply(self, v: &mut [f32]) {
        match self {
            Self::Tanh => v.iter_mut().for_each(|x| *x = tanh(*x)),
            Self::Logistic => v.iter_mut().for_each(|x| *x = logistic(*x)),
            Self::PosClip => v.iter_mut().for_each(|x| *x = x.clamp(0.0, 1.0)),
            Self::SymClip => v.iter_mut().for_each(|x| *x = x.clamp(-1.0, 1.0)),
            Self::Relu => v.iter_mut().for_each(|x| {
                if *x <= 0.0 {
                    *x = 0.0;
                }
            }),
            Self::Softmax => softmax_in_place(v),
            Self::Linear => {}
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FullyConnected {
    activation: Activation,
    /// `NT_SOFTMAX` writes float outputs; the others keep int mode.
    float_output: bool,
    no: usize,
    weights: Weights,
}

impl FullyConnected {
    fn step(&self, input: &[i8], out: &mut [f32]) {
        self.weights.dot(input, out);
        self.activation.apply(out);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Lstm {
    summary: bool,
    ni: usize,
    ns: usize,
    nf: usize,
    no: usize,
    /// CI, GI, GF1, GO.
    gates: [Weights; 4],
    softmax: Option<(FullyConnected, bool)>,
}

/// One layer of the network.
#[derive(Debug, Clone)]
pub(crate) enum Layer {
    Input {
        height: usize,
    },
    Convolve {
        ni: usize,
        half_x: usize,
        half_y: usize,
    },
    Maxpool {
        ni: usize,
        x_scale: usize,
        y_scale: usize,
    },
    Reconfig {
        ni: usize,
        x_scale: usize,
        y_scale: usize,
    },
    Series(Vec<Layer>),
    Parallel {
        no: usize,
        stack: Vec<Layer>,
    },
    XReversed(Box<Layer>),
    YReversed(Box<Layer>),
    XyTranspose(Box<Layer>),
    FullyConnected(Box<FullyConnected>),
    Lstm(Box<Lstm>),
}

/// The fields of the LSTM component around the network that the recognizer
/// needs (`LSTMRecognizer::DeSerialize`).
pub(crate) struct LstmComponent {
    pub(crate) network: Layer,
    pub(crate) null_char: i32,
    pub(crate) sample_iteration: i32,
    pub(crate) training_flags: i32,
}

impl LstmComponent {
    /// Reads the network and the recognizer fields that follow it. The
    /// unicharset and recoder come from their own components.
    pub(crate) fn read(data: &[u8]) -> Result<Self, LoadError> {
        let mut r = Reader::new(data);
        let network = Layer::read(&mut r, 0)?;
        let _spec = r.string()?;
        let training_flags = r.i32()?;
        let _training_iteration = r.i32()?;
        let sample_iteration = r.i32()?;
        let null_char = r.i32()?;
        let _adam_beta = r.f32()?;
        let _learning_rate = r.f32()?;
        let _momentum = r.f32()?;
        Ok(Self {
            network,
            null_char,
            sample_iteration,
            training_flags,
        })
    }
}

impl Layer {
    fn read(r: &mut Reader<'_>, depth: usize) -> Result<Self, LoadError> {
        if depth > MAX_DEPTH {
            return Err(LoadError::NoLstm);
        }
        let mut kind = r.i8()?;
        if kind == 0 {
            let name = r.string()?;
            kind = TYPE_NAMES
                .iter()
                .position(|n| *n == name)
                .ok_or(LoadError::UnsupportedLayer(name))? as i8;
        }
        let kind = usize::try_from(kind).map_err(|_| LoadError::NoLstm)?;
        let type_name = TYPE_NAMES.get(kind).ok_or(LoadError::NoLstm)?;
        let _training = r.i8()?;
        let _needs_backprop = r.i8()?;
        let flags = r.i32()?;
        let ni = r.i32()?;
        let no = r.i32()?;
        let _num_weights = r.i32()?;
        let _name = r.string()?;
        let (ni, no) = match (usize::try_from(ni), usize::try_from(no)) {
            (Ok(ni), Ok(no)) => (ni, no),
            _ => return Err(LoadError::NoLstm),
        };
        let bad = || LoadError::NoLstm;
        let pos = |v: i32| usize::try_from(v).ok().filter(|&v| v > 0).ok_or_else(bad);
        let layer = match kind {
            nt::INPUT => {
                let _batch = r.i32()?;
                let height = r.i32()?;
                let _width = r.i32()?;
                let depth = r.i32()?;
                let _loss = r.i32()?;
                // Tesseract's models take one grey channel; colour or 1-D
                // inputs are not used by any fast model.
                if depth != 1 || height <= 0 {
                    return Err(LoadError::UnsupportedLayer(format!("{type_name} shape")));
                }
                Layer::Input {
                    height: height as usize,
                }
            }
            nt::CONVOLVE => {
                let half_x = usize::try_from(r.i32()?).map_err(|_| bad())?;
                let half_y = usize::try_from(r.i32()?).map_err(|_| bad())?;
                if ni == 0 || half_x > 64 || half_y > 64 {
                    return Err(bad());
                }
                Layer::Convolve { ni, half_x, half_y }
            }
            nt::MAXPOOL | nt::RECONFIG => {
                let x_scale = pos(r.i32()?)?;
                let y_scale = pos(r.i32()?)?;
                if ni == 0 || x_scale > 64 || y_scale > 64 {
                    return Err(bad());
                }
                if kind == nt::MAXPOOL {
                    Layer::Maxpool {
                        ni,
                        x_scale,
                        y_scale,
                    }
                } else {
                    Layer::Reconfig {
                        ni,
                        x_scale,
                        y_scale,
                    }
                }
            }
            nt::SERIES
            | nt::PARALLEL
            | nt::REPLICATED
            | nt::PAR_RL_LSTM
            | nt::PAR_UD_LSTM
            | nt::XREVERSED
            | nt::YREVERSED
            | nt::XYTRANSPOSE => {
                let n = r.u32()? as usize;
                if n == 0 || n > 10_000 || (kind == nt::SERIES && n == 1) {
                    return Err(bad());
                }
                let mut stack = Vec::new();
                for _ in 0..n {
                    stack.push(Layer::read(r, depth + 1)?);
                }
                if flags & NF_LAYER_SPECIFIC_LR != 0 {
                    let count = r.count(4)?;
                    r.bytes(count * 4)?;
                }
                match kind {
                    nt::SERIES => Layer::Series(stack),
                    nt::XREVERSED | nt::YREVERSED | nt::XYTRANSPOSE => {
                        if stack.len() != 1 {
                            return Err(bad());
                        }
                        let inner = Box::new(stack.remove(0));
                        match kind {
                            nt::XREVERSED => Layer::XReversed(inner),
                            nt::YREVERSED => Layer::YReversed(inner),
                            _ => Layer::XyTranspose(inner),
                        }
                    }
                    _ => {
                        let no = stack.iter().map(Layer::outputs).sum();
                        Layer::Parallel { no, stack }
                    }
                }
            }
            nt::LOGISTIC
            | nt::POSCLIP
            | nt::SYMCLIP
            | nt::TANH
            | nt::RELU
            | nt::LINEAR
            | nt::SOFTMAX
            | nt::SOFTMAX_NO_CTC => {
                Layer::FullyConnected(Box::new(FullyConnected::read(r, kind, ni, no)?))
            }
            nt::LSTM | nt::LSTM_SUMMARY | nt::LSTM_SOFTMAX | nt::LSTM_SOFTMAX_ENCODED => {
                Layer::Lstm(Box::new(Lstm::read(r, kind, ni, no, depth)?))
            }
            _ => return Err(LoadError::UnsupportedLayer((*type_name).to_owned())),
        };
        Ok(layer)
    }

    /// Number of output features.
    pub(crate) fn outputs(&self) -> usize {
        match self {
            Layer::Input { .. } => 1,
            Layer::Convolve { ni, half_x, half_y } => ni * (2 * half_x + 1) * (2 * half_y + 1),
            Layer::Maxpool { ni, .. } => *ni,
            Layer::Reconfig {
                ni,
                x_scale,
                y_scale,
            } => ni * x_scale * y_scale,
            Layer::Series(stack) => stack.last().map_or(0, Layer::outputs),
            Layer::Parallel { no, .. } => *no,
            Layer::XReversed(inner) | Layer::YReversed(inner) | Layer::XyTranspose(inner) => {
                inner.outputs()
            }
            Layer::FullyConnected(fc) => fc.no,
            Layer::Lstm(lstm) => lstm.no,
        }
    }

    /// The input height of the first layer (`Network::NumInputs` of the
    /// input layer).
    pub(crate) fn input_height(&self) -> Option<usize> {
        match self {
            Layer::Input { height } => Some(*height),
            Layer::Series(stack) | Layer::Parallel { stack, .. } => {
                stack.first().and_then(Layer::input_height)
            }
            Layer::XReversed(inner) | Layer::YReversed(inner) | Layer::XyTranspose(inner) => {
                inner.input_height()
            }
            _ => None,
        }
    }

    /// `Network::XScaleFactor`: how many input columns make one output step.
    pub(crate) fn x_scale_factor(&self) -> usize {
        match self {
            Layer::Maxpool { x_scale, .. } | Layer::Reconfig { x_scale, .. } => *x_scale,
            Layer::Series(stack) => stack.iter().map(Layer::x_scale_factor).product(),
            Layer::Parallel { stack, .. } => stack.first().map_or(1, Layer::x_scale_factor),
            Layer::XReversed(inner) | Layer::YReversed(inner) | Layer::XyTranspose(inner) => {
                inner.x_scale_factor()
            }
            _ => 1,
        }
    }

    /// Checks that every layer's input width matches what feeds it, so the
    /// forward pass cannot index out of range. `ni` is the number of input
    /// features the layer receives; returns its output features.
    pub(crate) fn validate(&self, ni: usize) -> Result<usize, LoadError> {
        let bad = Err(LoadError::NoLstm);
        match self {
            Layer::Input { .. } => Ok(ni),
            Layer::Convolve { ni: own, .. } | Layer::Reconfig { ni: own, .. } => {
                if *own != ni {
                    bad
                } else {
                    Ok(self.outputs())
                }
            }
            Layer::Maxpool { ni: own, .. } => {
                if *own != ni {
                    bad
                } else {
                    Ok(ni)
                }
            }
            Layer::Series(stack) => stack.iter().try_fold(ni, |n, l| l.validate(n)),
            Layer::Parallel { stack, .. } => {
                let mut total = 0;
                for l in stack {
                    total += l.validate(ni)?;
                }
                Ok(total)
            }
            Layer::XReversed(inner) | Layer::YReversed(inner) | Layer::XyTranspose(inner) => {
                inner.validate(ni)
            }
            Layer::FullyConnected(fc) => {
                if fc.weights.inputs() != ni {
                    bad
                } else {
                    Ok(fc.no)
                }
            }
            Layer::Lstm(lstm) => {
                if lstm.ni != ni {
                    bad
                } else {
                    Ok(lstm.no)
                }
            }
        }
    }

    /// Runs the layer on `input` (`Network::Forward`).
    pub(crate) fn forward(&self, input: &NetworkIo, rand: &mut TRand) -> NetworkIo {
        match self {
            Layer::Input { .. } => input.clone(),
            Layer::Convolve { ni, half_x, half_y } => {
                convolve(input, *ni, *half_x, *half_y, self.outputs(), rand)
            }
            Layer::Maxpool {
                x_scale, y_scale, ..
            } => maxpool(input, *x_scale, *y_scale),
            Layer::Reconfig {
                ni,
                x_scale,
                y_scale,
            } => reconfig(input, *ni, *x_scale, *y_scale, self.outputs()),
            Layer::Series(stack) => {
                let mut current = stack[0].forward(input, rand);
                for layer in &stack[1..] {
                    current = layer.forward(&current, rand);
                }
                current
            }
            Layer::Parallel { no, stack } => {
                let mut out: Option<NetworkIo> = None;
                let mut offset = 0;
                for layer in stack {
                    let result = layer.forward(input, rand);
                    let dest = out.get_or_insert_with(|| result.like(result.map, *no));
                    result.pack_into(dest, offset);
                    offset += result.features;
                }
                out.unwrap_or_else(|| input.clone())
            }
            Layer::XReversed(inner) => inner.forward(&input.x_reversed(), rand).x_reversed(),
            Layer::YReversed(inner) => inner.forward(&input.y_reversed(), rand).y_reversed(),
            Layer::XyTranspose(inner) => {
                inner.forward(&input.xy_transposed(), rand).xy_transposed()
            }
            Layer::FullyConnected(fc) => fully_connected(fc, input),
            Layer::Lstm(lstm) => lstm.forward(input),
        }
    }
}

fn convolve(
    input: &NetworkIo,
    ni: usize,
    half_x: usize,
    half_y: usize,
    no: usize,
    rand: &mut TRand,
) -> NetworkIo {
    let map = input.map;
    let mut out = input.like(map, no);
    let y_scale = 2 * half_y + 1;
    let (hx, hy) = (half_x as isize, half_y as isize);
    for t in 0..map.steps() {
        let (y, x) = ((t / map.width) as isize, (t % map.width) as isize);
        let mut out_ix = 0;
        for dx in -hx..=hx {
            let sx = x + dx;
            if sx < 0 || sx >= map.width as isize {
                out.randomize(t, out_ix, y_scale * ni, rand);
            } else {
                let mut out_iy = out_ix;
                for dy in -hy..=hy {
                    let sy = y + dy;
                    if sy < 0 || sy >= map.height as isize {
                        out.randomize(t, out_iy, ni, rand);
                    } else {
                        let src_t = sy as usize * map.width + sx as usize;
                        out.copy_part(t, out_iy, ni, input, src_t, 0);
                    }
                    out_iy += ni;
                }
            }
            out_ix += y_scale * ni;
        }
    }
    out
}

fn maxpool(input: &NetworkIo, x_scale: usize, y_scale: usize) -> NetworkIo {
    let src = input.map;
    let map = Map {
        height: src.height / y_scale,
        width: src.width / x_scale,
    };
    let mut out = input.like(map, input.features);
    for t in 0..map.steps() {
        let (y, x) = (t / map.width * y_scale, t % map.width * x_scale);
        out.copy_step(t, input, y * src.width + x);
        for dx in 0..x_scale {
            for dy in 0..y_scale {
                if x + dx < src.width && y + dy < src.height {
                    out.max_step(t, input, (y + dy) * src.width + x + dx);
                }
            }
        }
    }
    out
}

fn reconfig(input: &NetworkIo, ni: usize, x_scale: usize, y_scale: usize, no: usize) -> NetworkIo {
    let src = input.map;
    let map = Map {
        height: src.height / y_scale,
        width: src.width / x_scale,
    };
    let mut out = input.like(map, no);
    for t in 0..map.steps() {
        let (y, x) = (t / map.width * y_scale, t % map.width * x_scale);
        for dx in 0..x_scale {
            for dy in 0..y_scale {
                if x + dx < src.width && y + dy < src.height {
                    let src_t = (y + dy) * src.width + x + dx;
                    out.copy_part(t, (dx * y_scale + dy) * ni, ni, input, src_t, 0);
                }
            }
        }
    }
    out
}

impl FullyConnected {
    fn read(r: &mut Reader<'_>, kind: usize, ni: usize, no: usize) -> Result<Self, LoadError> {
        let activation = match kind {
            nt::LOGISTIC => Activation::Logistic,
            nt::POSCLIP => Activation::PosClip,
            nt::SYMCLIP => Activation::SymClip,
            nt::TANH => Activation::Tanh,
            nt::RELU => Activation::Relu,
            nt::LINEAR => Activation::Linear,
            _ => Activation::Softmax,
        };
        let weights = Weights::read(r)?;
        if weights.rows != no || weights.cols != ni + 1 {
            return Err(LoadError::NoLstm);
        }
        Ok(Self {
            activation,
            float_output: kind == nt::SOFTMAX,
            no,
            weights,
        })
    }
}

fn fully_connected(fc: &FullyConnected, input: &NetworkIo) -> NetworkIo {
    let mut out = if fc.float_output {
        NetworkIo::new_float(input.map, fc.no)
    } else {
        input.like(input.map, fc.no)
    };
    let mut line = vec![0.0f32; fc.no];
    for t in 0..input.steps() {
        fc.step(input.int_row(t), &mut line);
        out.write_part(t, 0, &line);
    }
    out
}

impl Lstm {
    fn read(
        r: &mut Reader<'_>,
        kind: usize,
        ni: usize,
        no: usize,
        depth: usize,
    ) -> Result<Self, LoadError> {
        let na = usize::try_from(r.i32()?).map_err(|_| LoadError::NoLstm)?;
        let nf = match kind {
            nt::LSTM_SOFTMAX => no,
            nt::LSTM_SOFTMAX_ENCODED => {
                // ceil_log2
                let mut bits = 0;
                while (1usize << bits) < no {
                    bits += 1;
                }
                bits
            }
            _ => 0,
        };
        let mut gates = Vec::with_capacity(4);
        for _ in 0..4 {
            gates.push(Weights::read(r)?);
        }
        let ns = gates[0].rows;
        if na == ni + nf + 2 * ns {
            return Err(LoadError::UnsupportedLayer("2-D LSTM".to_owned()));
        }
        if na != ni + nf + ns {
            return Err(LoadError::NoLstm);
        }
        if gates.iter().any(|g| g.rows != ns || g.cols != na + 1) {
            return Err(LoadError::NoLstm);
        }
        let summary = kind == nt::LSTM_SUMMARY;
        if (kind == nt::LSTM || summary) && ns != no {
            return Err(LoadError::NoLstm);
        }
        let softmax = if nf > 0 || kind == nt::LSTM_SOFTMAX || kind == nt::LSTM_SOFTMAX_ENCODED {
            let Layer::FullyConnected(fc) = Layer::read(r, depth + 1)? else {
                return Err(LoadError::NoLstm);
            };
            if fc.weights.inputs() != ns || fc.no != no {
                return Err(LoadError::NoLstm);
            }
            Some((*fc, kind == nt::LSTM_SOFTMAX_ENCODED))
        } else {
            None
        };
        let gates: [Weights; 4] = gates.try_into().map_err(|_| LoadError::NoLstm)?;
        Ok(Self {
            summary,
            ni,
            ns,
            nf,
            no,
            gates,
            softmax,
        })
    }

    /// `LSTM::Forward` for a 1-D LSTM in int mode.
    fn forward(&self, input: &NetworkIo) -> NetworkIo {
        let map = input.map;
        let (ni, ns, nf) = (self.ni, self.ns, self.nf);
        let na = ni + nf + ns;
        let mut out = if self.softmax.is_some() {
            NetworkIo::new_float(map, self.no)
        } else if self.summary {
            input.like(
                Map {
                    height: map.height,
                    width: 1,
                },
                self.no,
            )
        } else {
            input.like(map, self.no)
        };
        // One time step of the LSTM's own input: the layer input, the fed
        // back softmax and the previous output, rounded to int8.
        let mut source = NetworkIo::new_int(
            Map {
                height: 1,
                width: 1,
            },
            na,
        );
        let mut int_output = NetworkIo::new_int(
            Map {
                height: 1,
                width: 1,
            },
            ns,
        );
        let mut lines = [
            vec![0.0f32; ns],
            vec![0.0; ns],
            vec![0.0; ns],
            vec![0.0; ns],
        ];
        let mut state = vec![0.0f32; ns];
        let mut output = vec![0.0f32; ns];
        let mut softmax_out = vec![0.0f32; self.no];
        let mut dest_t = 0;
        for t in 0..map.steps() {
            source.copy_part(0, 0, ni, input, t, 0);
            if self.softmax.is_some() {
                source.write_part(0, ni, &softmax_out[..nf]);
            }
            source.write_part(0, ni + nf, &output);
            let row = source.int_row(0);
            for (g, line) in lines.iter_mut().enumerate() {
                self.gates[g].dot(row, line);
                if g == 0 {
                    line.iter_mut().for_each(|x| *x = tanh(*x));
                } else {
                    line.iter_mut().for_each(|x| *x = logistic(*x));
                }
            }
            let [ci, gi, gf, go] = &lines;
            for i in 0..ns {
                state[i] *= gf[i];
            }
            for i in 0..ns {
                state[i] += ci[i] * gi[i];
            }
            for s in state.iter_mut() {
                *s = s.clamp(-STATE_CLIP, STATE_CLIP);
            }
            for i in 0..ns {
                output[i] = tanh(state[i]) * go[i];
            }
            let last_x = t % map.width == map.width - 1;
            if let Some((softmax, encoded)) = &self.softmax {
                int_output.write_part(0, 0, &output);
                softmax.step(int_output.int_row(0), &mut softmax_out);
                out.write_part(t, 0, &softmax_out);
                if *encoded {
                    code_in_binary(self.no, nf, &mut softmax_out);
                }
            } else if self.summary {
                if last_x {
                    out.write_part(dest_t, 0, &output);
                    dest_t += 1;
                }
            } else {
                out.write_part(t, 0, &output);
            }
            if last_x {
                state.fill(0.0);
                output.fill(0.0);
            }
        }
        out
    }
}

/// `CodeInBinary`: replaces the first `nf` values by the bits of the arg-max.
fn code_in_binary(n: usize, nf: usize, v: &mut [f32]) {
    if nf == 0 || n < nf {
        return;
    }
    let mut index = 0;
    let mut best = v[0];
    for (i, &x) in v.iter().enumerate().take(n).skip(1) {
        if x > best {
            best = x;
            index = i;
        }
    }
    for (i, x) in v.iter_mut().enumerate().take(nf) {
        *x = if index & (1 << i) != 0 { 1.0 } else { 0.0 };
    }
}
