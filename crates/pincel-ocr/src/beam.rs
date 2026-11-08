//! The dictionary-free recode beam search: Tesseract's `RecodeBeamSearch`
//! (`src/lstm/recodebeam.cpp`) with `dict_ == nullptr`, as the recognizer
//! runs it with `lstm_use_matrix=0`.
//!
//! The heaps are a port of Tesseract's `GenericHeap`: the search walks the
//! raw heap arrays, so their exact order decides ties.

use crate::recoder::{Code, MAX_CODE_LEN, Recoder};
use crate::unicharset::Unicharset;

/// `RecodeBeamSearch::kBeamWidths`.
const BEAM_WIDTHS: [usize; MAX_CODE_LEN + 1] = [5, 10, 16, 16, 16, 16, 16, 16, 16, 16];
/// `kNumLengths`.
const NUM_LENGTHS: usize = MAX_CODE_LEN + 1;
/// Non-dictionary beams: `NC_COUNT * kNumLengths`. The dictionary beams
/// that follow them in Tesseract stay empty without a dictionary.
const NUM_BEAMS: usize = 3 * NUM_LENGTHS;
/// `RecodeBeamSearch::kMinCertainty`.
const MIN_CERTAINTY: f32 = -20.0;
/// `kDictRatio` in `lstmrecognizer.cpp`.
const DICT_RATIO: f64 = 2.25;
/// `kCertOffset` in `lstmrecognizer.cpp`.
const CERT_OFFSET: f64 = -0.085;
/// `UNICHAR_SPACE`.
pub(crate) const UNICHAR_SPACE: i32 = 0;
const INVALID: i32 = -1;

/// `NodeContinuation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cont {
    Anything = 0,
    OnlyDup = 1,
    NoDup = 2,
}

/// `TopNState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rank {
    Top2 = 0,
    TopN = 1,
    AlsoRan = 2,
}

fn beam_index(cont: Cont, length: usize) -> usize {
    cont as usize * NUM_LENGTHS + length
}

fn cont_of(index: usize) -> Cont {
    match (index / NUM_LENGTHS) % 3 {
        0 => Cont::Anything,
        1 => Cont::OnlyDup,
        _ => Cont::NoDup,
    }
}

/// `NetworkIO::ProbToCertainty`.
fn prob_to_certainty(prob: f32) -> f32 {
    // kMinProb = std::exp(kMinCertainty) as float.
    let min_prob = MIN_CERTAINTY.exp();
    if prob > min_prob {
        prob.ln()
    } else {
        MIN_CERTAINTY
    }
}

/// `prob_to_certainty(p) + cert_offset`, computed in `double` and stored as
/// `float` like the C++.
fn offset_certainty(prob: f32) -> f32 {
    (f64::from(prob_to_certainty(prob)) + CERT_OFFSET) as f32
}

/// Where a node lives: time step, beam and position in that beam's heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NodeRef {
    t: u32,
    beam: u8,
    pos: u8,
}

/// `RecodeNode` without the dictionary fields.
#[derive(Debug, Clone, Copy)]
struct Node {
    code: i32,
    unichar_id: i32,
    duplicate: bool,
    certainty: f32,
    score: f32,
    prev: Option<NodeRef>,
    code_hash: u64,
}

/// A node about to be pushed onto a beam: the arguments of Tesseract's
/// `PushDupOrNoDawgIfBetter` / `ContinueUnichar`.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    code: i32,
    unichar_id: i32,
    dup: bool,
    cert: f32,
    cont: Cont,
    prev: Option<NodeRef>,
}

/// A port of Tesseract's `GenericHeap` (a min-heap on the key).
#[derive(Debug, Clone)]
struct Heap<K: PartialOrd + Copy, V: Copy> {
    heap: Vec<(K, V)>,
}

impl<K: PartialOrd + Copy, V: Copy> Heap<K, V> {
    fn new() -> Self {
        Self { heap: Vec::new() }
    }

    fn len(&self) -> usize {
        self.heap.len()
    }

    fn peek_key(&self) -> K {
        self.heap[0].0
    }

    fn push(&mut self, entry: (K, V)) {
        let hole = self.heap.len();
        self.heap.push(entry);
        let hole = self.sift_up(hole, &entry);
        self.heap[hole] = entry;
    }

    fn pop(&mut self) -> Option<(K, V)> {
        let new_size = self.heap.len().checked_sub(1)?;
        let top = self.heap[0];
        if new_size > 0 {
            let hole_pair = self.heap[new_size];
            self.heap.truncate(new_size);
            let hole = self.sift_down(0, &hole_pair);
            self.heap[hole] = hole_pair;
        } else {
            self.heap.clear();
        }
        Some(top)
    }

    fn reshuffle(&mut self, index: usize) {
        let hole_pair = self.heap[index];
        let index = self.sift_down(index, &hole_pair);
        let index = self.sift_up(index, &hole_pair);
        self.heap[index] = hole_pair;
    }

    fn sift_up(&mut self, mut hole: usize, pair: &(K, V)) -> usize {
        while hole > 0 {
            let parent = hole.div_ceil(2) - 1;
            if pair.0 < self.heap[parent].0 {
                self.heap[hole] = self.heap[parent];
                hole = parent;
            } else {
                break;
            }
        }
        hole
    }

    fn sift_down(&mut self, mut hole: usize, pair: &(K, V)) -> usize {
        let size = self.heap.len();
        loop {
            let mut child = hole * 2 + 1;
            if child >= size {
                break;
            }
            if child + 1 < size && self.heap[child + 1].0 < self.heap[child].0 {
                child += 1;
            }
            if self.heap[child].0 < pair.0 {
                self.heap[hole] = self.heap[child];
                hole = child;
            } else {
                break;
            }
        }
        hole
    }
}

type Beam = Heap<f64, Node>;

/// The best path through the outputs, as unichars.
#[derive(Debug, Clone, Default)]
pub(crate) struct Path {
    /// Unichar ids, spaces included.
    pub(crate) unichar_ids: Vec<i32>,
    /// Certainty of each unichar.
    pub(crate) certs: Vec<f32>,
    /// Character boundaries in output steps: one more entry than ids.
    pub(crate) boundaries: Vec<i32>,
}

/// One recognized word from the path, before boxes are mapped to pixels.
#[derive(Debug, Clone)]
pub(crate) struct PathWord {
    pub(crate) ids: Vec<i32>,
    /// Character boundaries of the word's characters (output steps), one
    /// pair per character.
    pub(crate) spans: Vec<(i32, i32)>,
    /// `min(space certainty, word certainty)`, before Tesseract's ×7 scaling.
    pub(crate) certainty: f32,
}

/// The beam search over a softmax output.
pub(crate) struct BeamSearch<'a> {
    recoder: &'a Recoder,
    unicharset: &'a Unicharset,
    null_char: i32,
    beams: Vec<Vec<Beam>>,
    top_flags: Vec<Rank>,
    top_code: i32,
    second_code: i32,
}

impl<'a> BeamSearch<'a> {
    pub(crate) fn new(recoder: &'a Recoder, unicharset: &'a Unicharset, null_char: usize) -> Self {
        Self {
            recoder,
            unicharset,
            null_char: null_char as i32,
            beams: Vec::new(),
            top_flags: Vec::new(),
            top_code: -1,
            second_code: -1,
        }
    }

    fn node(&self, r: NodeRef) -> &Node {
        &self.beams[r.t as usize][usize::from(r.beam)].heap[usize::from(r.pos)].1
    }

    /// `RecodeBeamSearch::Decode`: `outputs` has one row of
    /// `num_outputs` probabilities per time step.
    pub(crate) fn decode(&mut self, outputs: &[f32], num_outputs: usize) {
        self.beams.clear();
        for (t, row) in outputs.chunks_exact(num_outputs).enumerate() {
            self.compute_top_n(row, BEAM_WIDTHS[0]);
            self.decode_step(row, t);
        }
    }

    /// `ComputeTopN`.
    fn compute_top_n(&mut self, outputs: &[f32], top_n: usize) {
        self.top_flags.clear();
        self.top_flags.resize(outputs.len(), Rank::AlsoRan);
        self.top_code = -1;
        self.second_code = -1;
        let mut heap: Heap<f32, i32> = Heap::new();
        for (i, &o) in outputs.iter().enumerate() {
            if heap.len() < top_n || o > heap.peek_key() {
                heap.push((o, i as i32));
                if heap.len() > top_n {
                    heap.pop();
                }
            }
        }
        while let Some((_, code)) = heap.pop() {
            if heap.len() > 1 {
                self.top_flags[code as usize] = Rank::TopN;
            } else {
                self.top_flags[code as usize] = Rank::Top2;
                if heap.len() == 0 {
                    self.top_code = code;
                } else {
                    self.second_code = code;
                }
            }
        }
        self.top_flags[self.null_char as usize] = Rank::Top2;
    }

    /// `DecodeStep`.
    fn decode_step(&mut self, outputs: &[f32], t: usize) {
        let mut step: Vec<Beam> = (0..NUM_BEAMS).map(|_| Beam::new()).collect();
        if t == 0 {
            self.continue_context(
                None,
                beam_index(Cont::Anything, 0),
                outputs,
                Rank::Top2,
                &mut step,
            );
        } else {
            let mut total = 0;
            for tn in [Rank::Top2, Rank::TopN, Rank::AlsoRan] {
                if total != 0 {
                    break;
                }
                for index in 0..NUM_BEAMS {
                    for i in (0..self.beams[t - 1][index].len()).rev() {
                        let r = NodeRef {
                            t: (t - 1) as u32,
                            beam: index as u8,
                            pos: i as u8,
                        };
                        self.continue_context(Some(r), index, outputs, tn, &mut step);
                    }
                }
                total = (0..NUM_BEAMS)
                    .filter(|&i| cont_of(i) == Cont::Anything)
                    .map(|i| step[i].len())
                    .sum();
            }
        }
        self.beams.push(step);
    }

    /// `ContinueContext` without dictionary beams.
    fn continue_context(
        &self,
        prev: Option<NodeRef>,
        index: usize,
        outputs: &[f32],
        flag: Rank,
        step: &mut [Beam],
    ) {
        let length = index % NUM_LENGTHS;
        let prev_cont = cont_of(index);
        let mut prefix = Code::default();
        let mut full_code = Code::default();
        let mut previous = prev;
        for p in (0..length).rev() {
            let Some(mut r) = previous else { break };
            loop {
                let n = self.node(r);
                if !(n.duplicate || n.code == self.null_char) {
                    break;
                }
                match n.prev {
                    Some(pr) => r = pr,
                    None => return,
                }
            }
            let n = self.node(r);
            prefix.set(p, n.code);
            full_code.set(p, n.code);
            previous = n.prev;
        }
        let null = self.null_char;
        let prev_node = prev.map(|r| *self.node(r));
        if let Some(pn) = prev_node {
            if self.top_flags[pn.code as usize] == flag {
                if prev_cont != Cont::NoDup {
                    let cert = offset_certainty(outputs[pn.code as usize]);
                    self.push_dup(
                        length,
                        Candidate {
                            code: pn.code,
                            unichar_id: pn.unichar_id,
                            dup: true,
                            cert,
                            cont: Cont::Anything,
                            prev,
                        },
                        step,
                    );
                }
                if prev_cont == Cont::Anything && flag == Rank::Top2 && pn.code != null {
                    let cert = offset_certainty(outputs[pn.code as usize] + outputs[null as usize]);
                    self.push_dup(
                        length,
                        Candidate {
                            code: pn.code,
                            unichar_id: pn.unichar_id,
                            dup: true,
                            cert,
                            cont: Cont::NoDup,
                            prev,
                        },
                        step,
                    );
                }
            }
            if prev_cont == Cont::OnlyDup {
                return;
            }
            if pn.code != null && length > 0 && self.top_flags[null as usize] == flag {
                let cert = offset_certainty(outputs[null as usize]);
                self.push_dup(
                    length,
                    Candidate {
                        code: null,
                        unichar_id: INVALID,
                        dup: false,
                        cert,
                        cont: Cont::Anything,
                        prev,
                    },
                    step,
                );
            }
        }
        if let Some(finals) = self.recoder.final_codes(&prefix) {
            for &code in finals {
                if self.top_flags[code as usize] != flag {
                    continue;
                }
                if prev_node.is_some_and(|pn| pn.code == code) {
                    continue;
                }
                let mut cert = offset_certainty(outputs[code as usize]);
                if cert < MIN_CERTAINTY && code != null {
                    continue;
                }
                full_code.set(length, code);
                let mut unichar_id = self
                    .recoder
                    .decode(&full_code)
                    .map_or(INVALID, |u| u as i32);
                if length == 0 && code == null {
                    unichar_id = INVALID;
                }
                self.push_unichar(
                    Candidate {
                        code,
                        unichar_id,
                        dup: false,
                        cert,
                        cont: Cont::Anything,
                        prev,
                    },
                    step,
                );
                if flag == Rank::Top2 && code != null {
                    let prob = self.dup_prob(outputs, prev_node, prev_cont, code);
                    cert = offset_certainty(prob);
                    self.push_unichar(
                        Candidate {
                            code,
                            unichar_id,
                            dup: false,
                            cert,
                            cont: Cont::OnlyDup,
                            prev,
                        },
                        step,
                    );
                }
            }
        }
        if let Some(nexts) = self.recoder.next_codes(&prefix) {
            for &code in nexts {
                if self.top_flags[code as usize] != flag {
                    continue;
                }
                if prev_node.is_some_and(|pn| pn.code == code) {
                    continue;
                }
                let cert = offset_certainty(outputs[code as usize]);
                self.push_dup(
                    length + 1,
                    Candidate {
                        code,
                        unichar_id: INVALID,
                        dup: false,
                        cert,
                        cont: Cont::Anything,
                        prev,
                    },
                    step,
                );
                if flag == Rank::Top2 && code != null {
                    let prob = self.dup_prob(outputs, prev_node, prev_cont, code);
                    let cert = offset_certainty(prob);
                    self.push_dup(
                        length + 1,
                        Candidate {
                            code,
                            unichar_id: INVALID,
                            dup: false,
                            cert,
                            cont: Cont::OnlyDup,
                            prev,
                        },
                        step,
                    );
                }
            }
        }
    }

    /// The probability of `code` merged with the null char, plus the
    /// previous code when the two swap first and second place.
    fn dup_prob(&self, outputs: &[f32], prev: Option<Node>, prev_cont: Cont, code: i32) -> f32 {
        let null = self.null_char;
        let mut prob = outputs[code as usize] + outputs[null as usize];
        if let Some(pn) = prev
            && prev_cont == Cont::Anything
            && pn.code != null
            && ((pn.code == self.top_code && code == self.second_code)
                || (code == self.top_code && pn.code == self.second_code))
        {
            prob += outputs[pn.code as usize];
        }
        prob
    }

    /// `ContinueUnichar` without dictionary.
    fn push_unichar(&self, c: Candidate, step: &mut [Beam]) {
        let c = Candidate {
            cert: c.cert * DICT_RATIO as f32,
            ..c
        };
        self.push_heap(BEAM_WIDTHS[0], c, &mut step[beam_index(c.cont, 0)]);
    }

    /// `PushDupOrNoDawgIfBetter` without dictionary.
    fn push_dup(&self, length: usize, c: Candidate, step: &mut [Beam]) {
        let c = Candidate {
            cert: c.cert * DICT_RATIO as f32,
            ..c
        };
        if c.cert >= MIN_CERTAINTY || c.code == self.null_char {
            let index = beam_index(c.cont, length);
            self.push_heap(BEAM_WIDTHS[length], c, &mut step[index]);
        }
    }

    /// `PushHeapIfBetter`.
    fn push_heap(&self, max_size: usize, c: Candidate, heap: &mut Beam) {
        let prev_node = c.prev.map(|r| self.node(r));
        let mut score = c.cert;
        if let Some(pn) = prev_node {
            score += pn.score;
        }
        if heap.len() < max_size || score > heap.heap[0].1.score {
            let mut hash = prev_node.map_or(0, |pn| pn.code_hash);
            if !c.dup && c.code != self.null_char {
                let n = self.recoder.code_range() as u64;
                let carry = ((hash >> 32).wrapping_mul(n)) >> 32;
                hash = hash
                    .wrapping_mul(n)
                    .wrapping_add(carry)
                    .wrapping_add(c.code as u64);
            }
            let node = Node {
                code: c.code,
                unichar_id: c.unichar_id,
                duplicate: c.dup,
                certainty: c.cert,
                score,
                prev: c.prev,
                code_hash: hash,
            };
            // UpdateHeapIfMatched: permuter and dawg start are the same for
            // every node here.
            if let Some(i) = heap
                .heap
                .iter()
                .position(|(_, n)| n.code == node.code && n.code_hash == node.code_hash)
            {
                if node.score > heap.heap[i].1.score {
                    heap.heap[i] = (f64::from(node.score), node);
                    heap.reshuffle(i);
                }
                return;
            }
            heap.push((f64::from(score), node));
            if heap.len() > max_size {
                heap.pop();
            }
        }
    }

    /// `ExtractBestPaths` (best path only) and `ExtractPathAsUnicharIds`
    /// with the character boundaries.
    pub(crate) fn best_path(&self) -> Path {
        let Some(last) = self.beams.last() else {
            return Path::default();
        };
        let t_last = (self.beams.len() - 1) as u32;
        let mut best: Option<NodeRef> = None;
        for cont in [Cont::Anything, Cont::NoDup] {
            let index = beam_index(cont, 0);
            for h in 0..last[index].len() {
                let r = NodeRef {
                    t: t_last,
                    beam: index as u8,
                    pos: h as u8,
                };
                if best.is_none_or(|b| self.node(r).score > self.node(b).score) {
                    best = Some(r);
                }
            }
        }
        let mut nodes = Vec::new();
        let mut cur = best;
        while let Some(r) = cur {
            let n = *self.node(r);
            nodes.push(n);
            cur = n.prev;
        }
        nodes.reverse();
        path_as_unichars(&nodes)
    }

    /// `ExtractBestPathAsWords` up to the per-character boxes: splits the
    /// path into words at spaces and between characters of scripts that do
    /// not separate words with spaces.
    pub(crate) fn words(&self, path: &Path) -> Vec<PathWord> {
        let ids = &path.unichar_ids;
        let n = ids.len();
        let mut words = Vec::new();
        let mut prev_space_cert = 0.0f32;
        let mut start = 0;
        while start < n {
            let mut end = start + 1;
            while end < n {
                if ids[end] == UNICHAR_SPACE {
                    break;
                }
                if !self.unicharset.is_space_delimited(ids[end] as usize)
                    || !self.unicharset.is_space_delimited(ids[end - 1] as usize)
                {
                    break;
                }
                end += 1;
            }
            let space_cert = if end < n && ids[end] == UNICHAR_SPACE {
                path.certs[end]
            } else {
                0.0
            };
            let mut word_cert = f32::MAX;
            for &c in &path.certs[start..end] {
                if c < word_cert {
                    word_cert = c;
                }
            }
            let spans = (start..end)
                .filter(|&i| i + 1 < path.boundaries.len())
                .map(|i| (path.boundaries[i], path.boundaries[i + 1]))
                .collect();
            words.push(PathWord {
                ids: ids[start..end].to_vec(),
                spans,
                certainty: space_cert.min(prev_space_cert).min(word_cert),
            });
            prev_space_cert = space_cert;
            if end < n && ids[end] == UNICHAR_SPACE {
                end += 1;
            }
            start = end;
        }
        words
    }
}

/// `ExtractPathAsUnicharIds` plus `calculateCharBoundaries`.
fn path_as_unichars(nodes: &[Node]) -> Path {
    let width = nodes.len();
    let mut ids = Vec::new();
    let mut certs: Vec<f32> = Vec::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut t = 0;
    while t < width {
        let mut certainty = 0.0f64;
        while t < width && nodes[t].unichar_id == INVALID {
            let cert = f64::from(nodes[t].certainty);
            t += 1;
            if cert < certainty {
                certainty = cert;
            }
        }
        starts.push(t as i32);
        if t < width {
            let unichar_id = nodes[t].unichar_id;
            // Every node's permuter is TOP_CHOICE_PERM without a dictionary.
            if unichar_id == UNICHAR_SPACE && !certs.is_empty() {
                if let Some(last) = certs.last_mut()
                    && certainty < f64::from(*last)
                {
                    *last = certainty as f32;
                }
                certainty = 0.0;
            }
            ids.push(unichar_id);
            loop {
                let cert = f64::from(nodes[t].certainty);
                t += 1;
                if cert < certainty {
                    certainty = cert;
                }
                if !(t < width && nodes[t].duplicate) {
                    break;
                }
            }
            ends.push(t as i32);
            certs.push(certainty as f32);
        } else if let Some(last) = certs.last_mut()
            && certainty < f64::from(*last)
        {
            *last = certainty as f32;
        }
    }
    starts.push(width as i32);
    let mut boundaries = vec![0];
    for i in 0..ends.len() {
        let middle = (starts[i + 1] - ends[i]) / 2;
        boundaries.push(ends[i] + middle);
    }
    boundaries.pop();
    boundaries.push(width as i32);
    Path {
        unichar_ids: ids,
        certs,
        boundaries,
    }
}
