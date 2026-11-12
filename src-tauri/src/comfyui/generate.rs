//! Generating images with the connected ComfyUI (docs/specs/comfyui-generate.md):
//! a job uploads the input and the mask, queues one of the built-in
//! workflow templates ([`workflow`]), follows it over the progress socket
//! (or by polling), fetches the variants and keeps them until the editor
//! releases them. Cancel touches only the job's own prompt (AC-09).
//!
//! Jobs run on their own threads; their events go to the webview through
//! the `emit` callback ([`GenEvent`]).

mod job;
mod socket;
pub mod workflow;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::api::ServerInfo;
use super::http;
use workflow::Family;

/// Most variants one generation asks for (AC-03).
pub const MAX_VARIANTS: u32 = 8;

/// The largest generation side Pincel asks for (4:1 of SDXL's 1024²).
pub const MAX_SIDE: u32 = 4096;

/// What the Generate panel (or `gen.image`) asks for.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    /// A checkpoint of the server.
    pub model: String,
    pub family: Family,
    /// A LoRA of the server, or none.
    pub lora: Option<String>,
    /// 0–2.
    pub lora_strength: f64,
    pub prompt: String,
    pub negative: String,
    /// The first variant's seed; random when absent.
    pub seed: Option<u64>,
    /// 1–8 variants.
    pub count: u32,
    /// 0.01–1.0; below 1 the input is refined.
    pub strength: f64,
    /// The generation size, multiples of 64.
    pub width: u32,
    pub height: u32,
    /// The input fitted to the generation size, PNG in base64.
    pub input: Option<String>,
    /// The mask fitted to the generation size, PNG in base64 (white =
    /// generate); only with an input.
    pub mask: Option<String>,
}

/// A started job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub job: u64,
    /// The first variant's seed; variant `i` has `seed + i`.
    pub seed: u64,
}

/// Why a generation failed or came back short (AC-15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GenError {
    /// No usable server: the reason of `require_usable` (AC-02, AC-19).
    NotUsable { message: String },
    /// A value out of range or a request ComfyUI refused.
    BadParameters { message: String },
    /// The server does not have this checkpoint or LoRA.
    MissingModel { name: String },
    /// The server stopped answering.
    Unreachable,
    /// The GPU ran out of memory.
    OutOfMemory,
    /// The workflow failed on the server.
    Failed { message: String },
    /// The server answered something Pincel cannot read.
    BadAnswer,
    /// Fewer images than requested came back (`got` may be 0).
    Fewer { got: u32, wanted: u32 },
}

/// What a job reports to the webview (`comfyui:generation`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GenEvent {
    /// Waiting in the server's queue behind `ahead` prompts.
    Queued { job: u64, ahead: u32 },
    /// The server started this job.
    Running { job: u64 },
    /// Sampler step `step` of `total` (all variants together).
    Progress { job: u64, step: u32, total: u32 },
    /// A live preview, as a data URL.
    Preview { job: u64, image: String },
    /// The job ended with `count` variants (fetch them with
    /// [`Generations::variant`]); `error` when it came back short.
    Finished {
        job: u64,
        count: u32,
        error: Option<GenError>,
    },
    /// The job failed without variants.
    Failed { job: u64, error: GenError },
    /// The job was cancelled; nothing came back.
    Cancelled { job: u64 },
}

/// How a job waits.
#[derive(Debug, Clone, Copy)]
pub struct GenConfig {
    /// A socket read or a poll waits this long.
    pub poll: Duration,
    /// How often a waiting job asks for its queue position.
    pub queue_every: Duration,
    /// Each HTTP request (uploads, prompt, results).
    pub request: Duration,
    /// Connecting the progress socket.
    pub connect: Duration,
}

impl Default for GenConfig {
    fn default() -> Self {
        Self {
            poll: Duration::from_millis(250),
            queue_every: Duration::from_secs(1),
            request: Duration::from_secs(60),
            connect: Duration::from_secs(2),
        }
    }
}

/// Event listener (the Tauri event in the app).
pub type Emit = Box<dyn Fn(&GenEvent) + Send + Sync>;
/// Told whether any job runs (the managed install's guard, comfyui-install
/// AC-14).
pub type Busy = Box<dyn Fn(bool) + Send + Sync>;

#[derive(Default)]
struct Job {
    cancel: Arc<AtomicBool>,
    running: bool,
    /// Released while running: its variants are dropped on arrival.
    released: bool,
    variants: Vec<Vec<u8>>,
}

struct Inner {
    config: GenConfig,
    emit: Emit,
    busy: Busy,
    next: AtomicU64,
    jobs: Mutex<HashMap<u64, Job>>,
}

/// The app's generation jobs.
#[derive(Clone)]
pub struct Generations(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A random seed below 2³² (exact in JavaScript), from the standard
/// library's OS-seeded hash keys.
fn random_seed() -> u64 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(0x5eed);
    h.finish() & 0xffff_ffff
}

fn bad(message: impl Into<String>) -> GenError {
    GenError::BadParameters {
        message: message.into(),
    }
}

/// The request checked against the server, with the images decoded.
fn check(server: &ServerInfo, req: &GenerateRequest) -> Result<job::Plan, GenError> {
    if !server.checkpoints.contains(&req.model) {
        return Err(GenError::MissingModel {
            name: req.model.clone(),
        });
    }
    if let Some(lora) = &req.lora
        && !server.loras.contains(lora)
    {
        return Err(GenError::MissingModel { name: lora.clone() });
    }
    if req.prompt.trim().is_empty() {
        return Err(bad("the prompt is empty"));
    }
    if !(1..=MAX_VARIANTS).contains(&req.count) {
        return Err(bad(format!("count must be 1 to {MAX_VARIANTS}")));
    }
    if !(0.01..=1.0).contains(&req.strength) {
        return Err(bad("strength must be 0.01 to 1.0"));
    }
    if !(0.0..=2.0).contains(&req.lora_strength) {
        return Err(bad("lora_strength must be 0 to 2"));
    }
    for side in [req.width, req.height] {
        if !(64..=MAX_SIDE).contains(&side) || side % 64 != 0 {
            return Err(bad("the generation size must be multiples of 64"));
        }
    }
    let decode = |text: &Option<String>| -> Result<Option<Vec<u8>>, GenError> {
        text.as_deref()
            .map(|t| pincel_agent::base64::decode(t).ok_or_else(|| bad("an image is not base64")))
            .transpose()
    };
    let input = decode(&req.input)?;
    let mask = decode(&req.mask)?;
    if mask.is_some() && input.is_none() {
        return Err(bad("a mask needs an input image"));
    }
    let url = http::parse_url(&server.url).ok_or(GenError::BadAnswer)?;
    Ok(job::Plan {
        url,
        model: req.model.clone(),
        family: req.family,
        lora: req.lora.clone().map(|l| (l, req.lora_strength)),
        prompt: req.prompt.clone(),
        negative: req.negative.clone(),
        seed: req.seed.unwrap_or_else(random_seed),
        count: req.count,
        strength: req.strength,
        width: req.width,
        height: req.height,
        input,
        mask,
    })
}

impl Generations {
    pub fn new(config: GenConfig, emit: Emit, busy: Busy) -> Self {
        Self(Arc::new(Inner {
            config,
            emit,
            busy,
            next: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
        }))
    }

    /// Checks `req` against `server` and starts it on its own thread.
    pub fn start(&self, server: &ServerInfo, req: &GenerateRequest) -> Result<Started, GenError> {
        let plan = check(server, req)?;
        let id = self.0.next.fetch_add(1, Ordering::SeqCst);
        let seed = plan.seed;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = lock(&self.0.jobs);
            jobs.insert(
                id,
                Job {
                    cancel: Arc::clone(&cancel),
                    running: true,
                    ..Job::default()
                },
            );
            if jobs.values().filter(|j| j.running).count() == 1 {
                (self.0.busy)(true);
            }
        }
        let this = self.clone();
        std::thread::spawn(move || {
            let emit = |event: GenEvent| (this.0.emit)(&event);
            let outcome = job::run(&plan, id, &cancel, &this.0.config, &emit);
            this.finish(id, outcome);
        });
        Ok(Started { job: id, seed })
    }

    fn finish(&self, id: u64, outcome: Result<job::Outcome, GenError>) {
        let event = {
            let mut jobs = lock(&self.0.jobs);
            let event = match outcome {
                Ok(job::Outcome::Cancelled) => {
                    jobs.remove(&id);
                    GenEvent::Cancelled { job: id }
                }
                Err(error) => {
                    jobs.remove(&id);
                    GenEvent::Failed { job: id, error }
                }
                Ok(job::Outcome::Done(variants, error)) => {
                    let count = u32::try_from(variants.len()).unwrap_or(u32::MAX);
                    match jobs.get_mut(&id) {
                        // Released while finishing: nobody wants them.
                        None => GenEvent::Cancelled { job: id },
                        Some(job) if job.released => {
                            jobs.remove(&id);
                            GenEvent::Cancelled { job: id }
                        }
                        Some(job) => {
                            job.running = false;
                            job.variants = variants;
                            GenEvent::Finished {
                                job: id,
                                count,
                                error,
                            }
                        }
                    }
                }
            };
            if !jobs.values().any(|j| j.running) {
                (self.0.busy)(false);
            }
            event
        };
        (self.0.emit)(&event);
    }

    /// Cancels job `id`: removed from the queue, or stopped if it runs
    /// (AC-09). Its thread reports `Cancelled`.
    pub fn cancel(&self, id: u64) {
        if let Some(job) = lock(&self.0.jobs).get(&id) {
            job.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Variant `index` of the finished job `id`, PNG bytes.
    pub fn variant(&self, id: u64, index: usize) -> Option<Vec<u8>> {
        lock(&self.0.jobs)
            .get(&id)
            .and_then(|j| j.variants.get(index).cloned())
    }

    /// Forgets job `id` and its variants; a running one is cancelled.
    pub fn release(&self, id: u64) {
        let mut jobs = lock(&self.0.jobs);
        match jobs.get_mut(&id) {
            Some(job) if job.running => {
                job.released = true;
                job.cancel.store(true, Ordering::SeqCst);
            }
            Some(_) => {
                jobs.remove(&id);
            }
            None => {}
        }
    }

    /// Jobs still running.
    #[cfg(test)]
    pub fn running(&self) -> usize {
        lock(&self.0.jobs).values().filter(|j| j.running).count()
    }
}

/// A checkpoint with its model family when it is a catalog model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub name: String,
    pub family: Option<Family>,
}

/// The models a generation can use (the Generate panel, `gen.models`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Models {
    pub checkpoints: Vec<Checkpoint>,
    pub loras: Vec<String>,
}

/// The server's models, catalog checkpoints with their family (AC-03).
pub fn models(server: &ServerInfo) -> Models {
    Models {
        checkpoints: server
            .checkpoints
            .iter()
            .map(|name| Checkpoint {
                name: name.clone(),
                family: workflow::catalog_family(name),
            })
            .collect(),
        loras: server.loras.clone(),
    }
}
