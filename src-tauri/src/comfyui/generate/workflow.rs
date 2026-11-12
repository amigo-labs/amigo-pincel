//! The built-in workflow templates (docs/specs/comfyui-generate.md,
//! Technical notes): text to image, image to image and inpainting in
//! ComfyUI's API format, made of the core nodes `api::REQUIRED_NODES`
//! lists only.
//!
//! Each variant is its own sampler branch with seed `seed + i`, so every
//! variant can be reproduced from its seed alone (Decisions).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The model families Pincel has sampler presets for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Family {
    Sd15,
    Sdxl,
    Flux,
}

/// Sampler settings of a family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub steps: u32,
    pub cfg: f64,
    pub sampler: &'static str,
    pub scheduler: &'static str,
}

/// SD 1.5 and SDXL: 30 steps, CFG 7, `dpmpp_2m` / `karras`; Flux.1
/// schnell: 4 steps, CFG 1, `euler` / `simple`.
pub fn preset(family: Family) -> Preset {
    match family {
        Family::Sd15 | Family::Sdxl => Preset {
            steps: 30,
            cfg: 7.0,
            sampler: "dpmpp_2m",
            scheduler: "karras",
        },
        Family::Flux => Preset {
            steps: 4,
            cfg: 1.0,
            sampler: "euler",
            scheduler: "simple",
        },
    }
}

/// The family of a catalog checkpoint (comfyui-install), by file name;
/// `None` for checkpoints outside the catalog.
pub fn catalog_family(checkpoint: &str) -> Option<Family> {
    let file = checkpoint.rsplit(['/', '\\']).next().unwrap_or(checkpoint);
    super::super::pins::catalog()
        .into_iter()
        .find(|p| p.file == file)
        .and_then(|p| match p.id.as_str() {
            "sd15" => Some(Family::Sd15),
            "sdxl" => Some(Family::Sdxl),
            "flux-schnell" => Some(Family::Flux),
            _ => None,
        })
}

/// Everything a template needs; the image names are the uploaded files.
#[derive(Debug, Clone, PartialEq)]
pub struct Template<'a> {
    pub model: &'a str,
    pub family: Family,
    /// LoRA file and strength.
    pub lora: Option<(&'a str, f64)>,
    pub prompt: &'a str,
    pub negative: &'a str,
    pub seed: u64,
    pub count: u32,
    /// 0.01–1.0; below 1 the input is refined.
    pub strength: f64,
    pub width: u32,
    pub height: u32,
    /// The uploaded input image (image to image, inpainting).
    pub input: Option<&'a str>,
    /// The uploaded mask (white = generate), only with an input.
    pub mask: Option<&'a str>,
}

/// The id of variant `i`'s sampler node.
pub fn sampler_id(i: u32) -> String {
    format!("sample{i}")
}

/// The id of variant `i`'s `SaveImage` node.
pub fn save_id(i: u32) -> String {
    format!("save{i}")
}

/// The variant a sampler node id belongs to.
pub fn variant_of(node: &str) -> Option<u32> {
    node.strip_prefix("sample")?.parse().ok()
}

fn link(node: &str, output: u32) -> Value {
    json!([node, output])
}

/// The prompt (`POST /prompt`'s `prompt`) for `t`.
pub fn build(t: &Template<'_>) -> Value {
    let mut nodes = Map::new();
    let mut add = |id: &str, class: &str, inputs: Value| {
        nodes.insert(
            id.to_string(),
            json!({ "class_type": class, "inputs": inputs }),
        );
    };
    add(
        "ckpt",
        "CheckpointLoaderSimple",
        json!({ "ckpt_name": t.model }),
    );
    let (model, clip) = match t.lora {
        Some((name, strength)) => {
            add(
                "lora",
                "LoraLoader",
                json!({
                    "model": link("ckpt", 0),
                    "clip": link("ckpt", 1),
                    "lora_name": name,
                    "strength_model": strength,
                    "strength_clip": strength,
                }),
            );
            (link("lora", 0), link("lora", 1))
        }
        None => (link("ckpt", 0), link("ckpt", 1)),
    };
    let vae = link("ckpt", 2);
    add(
        "pos",
        "CLIPTextEncode",
        json!({ "text": t.prompt, "clip": clip }),
    );
    // Flux ignores the negative prompt (AC-03).
    let negative = if t.family == Family::Flux {
        ""
    } else {
        t.negative
    };
    add(
        "neg",
        "CLIPTextEncode",
        json!({ "text": negative, "clip": clip }),
    );
    let strength = t.strength.clamp(0.01, 1.0);
    let denoise = match (t.input, t.mask) {
        (None, _) => {
            add(
                "latent",
                "EmptyLatentImage",
                json!({ "width": t.width, "height": t.height, "batch_size": 1 }),
            );
            1.0
        }
        (Some(input), mask) => {
            add("input", "LoadImage", json!({ "image": input }));
            let pixels = link("input", 0);
            match mask {
                None => {
                    add(
                        "latent",
                        "VAEEncode",
                        json!({ "pixels": pixels, "vae": vae }),
                    );
                    strength
                }
                Some(mask) => {
                    add(
                        "mask",
                        "LoadImageMask",
                        json!({ "image": mask, "channel": "red" }),
                    );
                    if strength >= 1.0 {
                        add(
                            "latent",
                            "VAEEncodeForInpaint",
                            json!({
                                "pixels": pixels,
                                "vae": vae,
                                "mask": link("mask", 0),
                                "grow_mask_by": 6,
                            }),
                        );
                        1.0
                    } else {
                        add(
                            "encoded",
                            "VAEEncode",
                            json!({ "pixels": pixels, "vae": vae }),
                        );
                        add(
                            "latent",
                            "SetLatentNoiseMask",
                            json!({ "samples": link("encoded", 0), "mask": link("mask", 0) }),
                        );
                        strength
                    }
                }
            }
        }
    };
    let p = preset(t.family);
    for i in 0..t.count {
        let sampler = sampler_id(i);
        let decode = format!("decode{i}");
        add(
            &sampler,
            "KSampler",
            json!({
                "model": model,
                "seed": t.seed.wrapping_add(u64::from(i)),
                "steps": p.steps,
                "cfg": p.cfg,
                "sampler_name": p.sampler,
                "scheduler": p.scheduler,
                "positive": link("pos", 0),
                "negative": link("neg", 0),
                "latent_image": link("latent", 0),
                "denoise": denoise,
            }),
        );
        add(
            &decode,
            "VAEDecode",
            json!({ "samples": link(&sampler, 0), "vae": vae }),
        );
        add(
            &save_id(i),
            "SaveImage",
            json!({ "images": link(&decode, 0), "filename_prefix": "pincel" }),
        );
    }
    Value::Object(nodes)
}
