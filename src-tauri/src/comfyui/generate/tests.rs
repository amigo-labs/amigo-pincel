use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::fake::{FakeGen, Script};
use super::workflow::{self, Family, Template};
use super::*;

fn server(url: String) -> ServerInfo {
    ServerInfo {
        url,
        version: "0.3.40".into(),
        gpu: None,
        checkpoints: vec!["sd15.safetensors".into(), "sdxl.safetensors".into()],
        loras: vec!["pixel.safetensors".into()],
        missing_nodes: Vec::new(),
        started: false,
    }
}

fn fast() -> GenConfig {
    GenConfig {
        poll: Duration::from_millis(20),
        queue_every: Duration::from_millis(30),
        request: Duration::from_secs(5),
        connect: Duration::from_secs(2),
    }
}

type Events = Arc<Mutex<Vec<GenEvent>>>;

struct Rig {
    gens: Generations,
    events: Events,
    busy: Arc<Mutex<Vec<bool>>>,
}

fn rig() -> Rig {
    let events: Events = Arc::default();
    let busy: Arc<Mutex<Vec<bool>>> = Arc::default();
    let (e, b) = (Arc::clone(&events), Arc::clone(&busy));
    let gens = Generations::new(
        fast(),
        Box::new(move |ev| e.lock().unwrap().push(ev.clone())),
        Box::new(move |on| b.lock().unwrap().push(on)),
    );
    Rig { gens, events, busy }
}

impl Rig {
    /// Waits until an event matches `pred`; all events so far.
    fn wait(&self, pred: impl Fn(&GenEvent) -> bool) -> Vec<GenEvent> {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let events = self.events.lock().unwrap().clone();
            if events.iter().any(&pred) {
                return events;
            }
            assert!(Instant::now() < until, "no matching event in {events:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_end(&self) -> Vec<GenEvent> {
        self.wait(|e| {
            matches!(
                e,
                GenEvent::Finished { .. } | GenEvent::Failed { .. } | GenEvent::Cancelled { .. }
            )
        })
    }
}

fn request() -> GenerateRequest {
    GenerateRequest {
        model: "sd15.safetensors".into(),
        family: Family::Sd15,
        lora: None,
        lora_strength: 1.0,
        prompt: "a red apple on a table".into(),
        negative: "blurry".into(),
        seed: Some(7),
        count: 2,
        strength: 1.0,
        width: 512,
        height: 512,
        input: None,
        mask: None,
    }
}

fn template() -> Template<'static> {
    Template {
        model: "sd15.safetensors",
        family: Family::Sd15,
        lora: None,
        prompt: "apple",
        negative: "blurry",
        seed: 7,
        count: 3,
        strength: 1.0,
        width: 768,
        height: 512,
        input: None,
        mask: None,
    }
}

fn class_of(prompt: &Value, id: &str) -> Option<String> {
    prompt
        .pointer(&format!("/{id}/class_type"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn input_of(prompt: &Value, id: &str, key: &str) -> Value {
    prompt
        .pointer(&format!("/{id}/inputs/{key}"))
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn text_to_image_has_one_sampler_branch_per_variant_with_consecutive_seeds() {
    let p = workflow::build(&template());
    assert_eq!(class_of(&p, "latent").as_deref(), Some("EmptyLatentImage"));
    assert_eq!(input_of(&p, "latent", "width"), json!(768));
    assert_eq!(input_of(&p, "latent", "height"), json!(512));
    assert_eq!(class_of(&p, "input"), None);
    for i in 0..3u64 {
        let s = format!("sample{i}");
        assert_eq!(class_of(&p, &s).as_deref(), Some("KSampler"));
        assert_eq!(input_of(&p, &s, "seed"), json!(7 + i));
        assert_eq!(input_of(&p, &s, "denoise"), json!(1.0));
        assert_eq!(input_of(&p, &s, "steps"), json!(30));
        assert_eq!(input_of(&p, &s, "sampler_name"), json!("dpmpp_2m"));
        assert_eq!(input_of(&p, &s, "scheduler"), json!("karras"));
        assert_eq!(
            class_of(&p, &format!("save{i}")).as_deref(),
            Some("SaveImage")
        );
    }
    assert_eq!(class_of(&p, "sample3"), None);
    assert_eq!(input_of(&p, "neg", "text"), json!("blurry"));
}

#[test]
fn image_to_image_encodes_the_input_and_denoises_by_the_strength() {
    let p = workflow::build(&Template {
        strength: 0.4,
        input: Some("in.png"),
        ..template()
    });
    assert_eq!(class_of(&p, "input").as_deref(), Some("LoadImage"));
    assert_eq!(input_of(&p, "input", "image"), json!("in.png"));
    assert_eq!(class_of(&p, "latent").as_deref(), Some("VAEEncode"));
    assert_eq!(input_of(&p, "sample0", "denoise"), json!(0.4));
}

#[test]
fn inpainting_uses_vae_encode_for_inpaint_at_full_strength_and_a_noise_mask_below() {
    let full = workflow::build(&Template {
        input: Some("in.png"),
        mask: Some("mask.png"),
        ..template()
    });
    assert_eq!(class_of(&full, "mask").as_deref(), Some("LoadImageMask"));
    assert_eq!(input_of(&full, "mask", "channel"), json!("red"));
    assert_eq!(
        class_of(&full, "latent").as_deref(),
        Some("VAEEncodeForInpaint")
    );
    assert_eq!(input_of(&full, "sample0", "denoise"), json!(1.0));
    let refine = workflow::build(&Template {
        strength: 0.6,
        input: Some("in.png"),
        mask: Some("mask.png"),
        ..template()
    });
    assert_eq!(
        class_of(&refine, "latent").as_deref(),
        Some("SetLatentNoiseMask")
    );
    assert_eq!(class_of(&refine, "encoded").as_deref(), Some("VAEEncode"));
    assert_eq!(input_of(&refine, "sample1", "denoise"), json!(0.6));
}

#[test]
fn lora_feeds_model_and_clip_and_flux_drops_the_negative_prompt() {
    let p = workflow::build(&Template {
        lora: Some(("pixel.safetensors", 0.5)),
        ..template()
    });
    assert_eq!(
        input_of(&p, "lora", "lora_name"),
        json!("pixel.safetensors")
    );
    assert_eq!(input_of(&p, "lora", "strength_model"), json!(0.5));
    assert_eq!(input_of(&p, "lora", "strength_clip"), json!(0.5));
    assert_eq!(input_of(&p, "sample0", "model"), json!(["lora", 0]));
    assert_eq!(input_of(&p, "pos", "clip"), json!(["lora", 1]));
    let flux = workflow::build(&Template {
        family: Family::Flux,
        ..template()
    });
    assert_eq!(input_of(&flux, "neg", "text"), json!(""));
    assert_eq!(input_of(&flux, "sample0", "steps"), json!(4));
    assert_eq!(input_of(&flux, "sample0", "cfg"), json!(1.0));
    assert_eq!(input_of(&flux, "sample0", "sampler_name"), json!("euler"));
    assert_eq!(input_of(&flux, "sample0", "scheduler"), json!("simple"));
    // Only the core nodes comfyui-connect checks for.
    for node in p.as_object().unwrap().values() {
        let class = node.get("class_type").and_then(Value::as_str).unwrap();
        assert!(
            crate::comfyui::api::REQUIRED_NODES.contains(&class),
            "{class}"
        );
    }
}

#[test]
fn catalog_checkpoints_know_their_family() {
    assert_eq!(
        workflow::catalog_family("sd_xl_base_1.0.safetensors"),
        Some(Family::Sdxl)
    );
    assert_eq!(
        workflow::catalog_family("SD/v1-5-pruned-emaonly-fp16.safetensors"),
        Some(Family::Sd15)
    );
    assert_eq!(
        workflow::catalog_family("flux1-schnell-fp8.safetensors"),
        Some(Family::Flux)
    );
    assert_eq!(workflow::catalog_family("my-model.safetensors"), None);
    let m = models(&server("http://127.0.0.1:1".into()));
    assert_eq!(m.checkpoints[0].family, None);
    assert_eq!(m.loras, ["pixel.safetensors"]);
}

#[test]
fn a_job_uploads_queues_follows_and_keeps_the_variants() {
    let fake = FakeGen::start(Script::default());
    let r = rig();
    let req = GenerateRequest {
        strength: 0.5,
        input: Some(pincel_agent::base64::encode(b"input-png")),
        mask: Some(pincel_agent::base64::encode(b"mask-png")),
        ..request()
    };
    let started = r.gens.start(&server(fake.url()), &req).unwrap();
    assert_eq!(started.seed, 7);
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Finished {
            job: started.job,
            count: 2,
            error: None
        }),
        "{events:?}"
    );
    assert!(events.iter().any(|e| matches!(e, GenEvent::Running { .. })));
    assert!(events.contains(&GenEvent::Progress {
        job: started.job,
        step: 1,
        total: 60
    }));
    assert!(events.contains(&GenEvent::Progress {
        job: started.job,
        step: 60,
        total: 60
    }));
    assert!(
        events.iter().any(|e| matches!(e, GenEvent::Preview { image, .. } if image.starts_with("data:image/jpeg;base64,"))),
        "{events:?}"
    );
    let uploads = fake.uploads();
    assert_eq!(uploads.len(), 2);
    assert_eq!(uploads[0].1, b"input-png");
    assert_eq!(uploads[1].1, b"mask-png");
    let body = &fake.prompts()[0];
    let prompt = &body["prompt"];
    assert_eq!(input_of(prompt, "input", "image"), json!(uploads[0].0));
    assert_eq!(input_of(prompt, "mask", "image"), json!(uploads[1].0));
    assert_eq!(input_of(prompt, "sample0", "denoise"), json!(0.5));
    assert!(body["client_id"].as_str().unwrap().starts_with("pincel-"));
    assert_eq!(
        r.gens.variant(started.job, 0).as_deref(),
        Some(&b"PNG:pincel_0.png"[..])
    );
    assert_eq!(
        r.gens.variant(started.job, 1).as_deref(),
        Some(&b"PNG:pincel_1.png"[..])
    );
    assert_eq!(*r.busy.lock().unwrap(), [true, false]);
    r.gens.release(started.job);
    assert_eq!(r.gens.variant(started.job, 0), None);
}

#[test]
fn a_waiting_job_reports_how_many_are_ahead() {
    let fake = FakeGen::start(Script {
        hold: true,
        ahead: 2,
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    r.wait(|e| *e == GenEvent::Queued { job, ahead: 2 });
    assert_eq!(r.gens.running(), 1);
    fake.release();
    let events = r.wait_end();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GenEvent::Finished { count: 2, .. }))
    );
}

#[test]
fn cancel_deletes_a_waiting_job_and_leaves_other_jobs_alone() {
    let fake = FakeGen::start(Script::default());
    fake.queue_foreign("someone-else");
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    r.wait(|e| *e == GenEvent::Queued { job, ahead: 1 });
    r.gens.cancel(job);
    let events = r.wait_end();
    assert!(events.contains(&GenEvent::Cancelled { job }));
    assert_eq!(fake.deleted(), ["prompt-1"]);
    assert!(fake.interrupts().is_empty());
    assert_eq!(*r.busy.lock().unwrap(), [true, false]);
}

#[test]
fn cancel_interrupts_a_running_job_by_its_own_id() {
    let fake = FakeGen::start(Script {
        stall: true,
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    r.wait(|e| matches!(e, GenEvent::Progress { .. }));
    r.gens.cancel(job);
    let events = r.wait_end();
    assert!(events.contains(&GenEvent::Cancelled { job }));
    assert_eq!(fake.interrupts(), [json!({ "prompt_id": "prompt-1" })]);
    assert!(fake.deleted().is_empty());
}

#[test]
fn out_of_memory_keeps_the_variants_that_arrived() {
    let fake = FakeGen::start(Script {
        error: Some((
            "torch.OutOfMemoryError".into(),
            "CUDA out of memory.".into(),
        )),
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Finished {
            job,
            count: 1,
            error: Some(GenError::OutOfMemory)
        }),
        "{events:?}"
    );
}

#[test]
fn fewer_images_than_requested_finish_short_and_none_fail() {
    let fake = FakeGen::start(Script {
        images: Some(1),
        ..Script::default()
    });
    let r = rig();
    let job = r
        .gens
        .start(
            &server(fake.url()),
            &GenerateRequest {
                count: 3,
                ..request()
            },
        )
        .unwrap()
        .job;
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Finished {
            job,
            count: 1,
            error: Some(GenError::Fewer { got: 1, wanted: 3 })
        }),
        "{events:?}"
    );
    let none = FakeGen::start(Script {
        images: Some(0),
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(none.url()), &request()).unwrap().job;
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Failed {
            job,
            error: GenError::Fewer { got: 0, wanted: 2 }
        }),
        "{events:?}"
    );
}

#[test]
fn bad_values_and_unknown_models_are_refused_before_anything_is_sent() {
    let fake = FakeGen::start(Script::default());
    let r = rig();
    let info = server(fake.url());
    let refused = |req: GenerateRequest| r.gens.start(&info, &req).unwrap_err();
    assert_eq!(
        refused(GenerateRequest {
            model: "nope.safetensors".into(),
            ..request()
        }),
        GenError::MissingModel {
            name: "nope.safetensors".into()
        }
    );
    assert_eq!(
        refused(GenerateRequest {
            lora: Some("gone.safetensors".into()),
            ..request()
        }),
        GenError::MissingModel {
            name: "gone.safetensors".into()
        }
    );
    for bad in [
        GenerateRequest {
            count: 9,
            ..request()
        },
        GenerateRequest {
            strength: 0.0,
            ..request()
        },
        GenerateRequest {
            lora_strength: 2.5,
            ..request()
        },
        GenerateRequest {
            width: 500,
            ..request()
        },
        GenerateRequest {
            prompt: " ".into(),
            ..request()
        },
        GenerateRequest {
            mask: Some(pincel_agent::base64::encode(b"m")),
            ..request()
        },
    ] {
        assert!(
            matches!(refused(bad), GenError::BadParameters { .. }),
            "should be refused"
        );
    }
    assert!(fake.prompts().is_empty());
    assert!(r.busy.lock().unwrap().is_empty());
}

#[test]
fn a_model_the_server_refuses_is_reported_missing() {
    let fake = FakeGen::start(Script {
        reject: Some(
            json!({
                "error": { "type": "prompt_outputs_failed_validation", "message": "Prompt outputs failed validation" },
                "node_errors": { "ckpt": { "errors": [{ "type": "value_not_in_list", "message": "Value not in list", "details": "ckpt_name: 'sd15.safetensors' not in []" }] } }
            })
            .to_string(),
        ),
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Failed {
            job,
            error: GenError::MissingModel {
                name: "sd15.safetensors".into()
            }
        }),
        "{events:?}"
    );
}

#[test]
fn a_server_that_stops_mid_generation_is_unreachable() {
    let fake = FakeGen::start(Script {
        stall: true,
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    r.wait(|e| matches!(e, GenEvent::Progress { .. }));
    fake.die();
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Failed {
            job,
            error: GenError::Unreachable
        }),
        "{events:?}"
    );
}

#[test]
fn without_the_socket_a_job_is_polled_to_its_end() {
    let fake = FakeGen::start(Script {
        no_socket: true,
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    let events = r.wait_end();
    assert!(
        events.contains(&GenEvent::Finished {
            job,
            count: 2,
            error: None
        }),
        "{events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, GenEvent::Preview { .. })));
}

#[test]
fn releasing_a_running_job_cancels_it() {
    let fake = FakeGen::start(Script {
        stall: true,
        ..Script::default()
    });
    let r = rig();
    let job = r.gens.start(&server(fake.url()), &request()).unwrap().job;
    r.wait(|e| matches!(e, GenEvent::Progress { .. }));
    r.gens.release(job);
    let events = r.wait_end();
    assert!(events.contains(&GenEvent::Cancelled { job }));
    assert_eq!(r.gens.running(), 0);
    assert_eq!(fake.interrupts(), [json!({ "prompt_id": "prompt-1" })]);
}
