//! One generation job against ComfyUI's HTTP API and progress socket:
//! upload, queue, follow, fetch, cancel (docs/specs/comfyui-generate.md,
//! Technical notes).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde_json::{Value, json};

use super::super::http::{self, HttpError, Response, Url};
use super::socket::{Frame, Socket};
use super::workflow::{self, Family, Template};
use super::{GenConfig, GenError, GenEvent};

/// A checked request.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub url: Url,
    pub model: String,
    pub family: Family,
    pub lora: Option<(String, f64)>,
    pub prompt: String,
    pub negative: String,
    pub seed: u64,
    pub count: u32,
    pub strength: f64,
    pub width: u32,
    pub height: u32,
    pub input: Option<Vec<u8>>,
    pub mask: Option<Vec<u8>>,
}

/// How a job ended without an error of its own.
#[derive(Debug)]
pub enum Outcome {
    /// The variants that came back (PNG bytes), and why fewer came back
    /// than requested.
    Done(Vec<Vec<u8>>, Option<GenError>),
    Cancelled,
}

/// Where the job's prompt is on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Pending {
        ahead: u32,
    },
    Running,
    /// Neither waiting nor running: finished or failed.
    Gone,
}

/// What a socket event means for this job.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    Started,
    Progress { node: String, value: u32 },
    Done,
    Error { kind: String, message: String },
    Interrupted,
    Other,
}

fn unreachable(_: HttpError) -> GenError {
    GenError::Unreachable
}

fn post_json(url: &Url, path: &str, body: &Value, cfg: &GenConfig) -> Result<Response, GenError> {
    http::post(
        url,
        path,
        "application/json",
        body.to_string().into_bytes(),
        cfg.request,
    )
    .map_err(unreachable)
}

fn get_json(url: &Url, path: &str, cfg: &GenConfig) -> Result<Value, GenError> {
    let res = http::get(url, path, cfg.request).map_err(unreachable)?;
    if res.status != 200 {
        return Err(GenError::BadAnswer);
    }
    serde_json::from_slice(&res.body).map_err(|_| GenError::BadAnswer)
}

/// `POST /upload/image` as multipart form data; the name `LoadImage` takes.
fn upload(url: &Url, name: &str, png: &[u8], cfg: &GenConfig) -> Result<String, GenError> {
    const BOUNDARY: &str = "pincel-form-7d1c9b2e";
    let mut body = Vec::with_capacity(png.len() + 512);
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"{name}\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(png);
    body.extend_from_slice(
        format!(
            "\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"type\"\r\n\r\ninput\r\n--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"overwrite\"\r\n\r\ntrue\r\n--{BOUNDARY}--\r\n"
        )
        .as_bytes(),
    );
    let res = http::post(
        url,
        "/upload/image",
        &format!("multipart/form-data; boundary={BOUNDARY}"),
        body,
        cfg.request,
    )
    .map_err(unreachable)?;
    if res.status != 200 {
        return Err(GenError::BadAnswer);
    }
    let answer: Value = serde_json::from_slice(&res.body).map_err(|_| GenError::BadAnswer)?;
    let stored = answer
        .get("name")
        .and_then(Value::as_str)
        .ok_or(GenError::BadAnswer)?;
    Ok(match answer.get("subfolder").and_then(Value::as_str) {
        Some(sub) if !sub.is_empty() => format!("{sub}/{stored}"),
        _ => stored.to_string(),
    })
}

/// A refused `POST /prompt`: a missing checkpoint or LoRA, or another
/// validation error.
fn refused(plan: &Plan, res: &Response) -> GenError {
    let Ok(answer) = serde_json::from_slice::<Value>(&res.body) else {
        return GenError::BadAnswer;
    };
    let mut details = Vec::new();
    if let Some(nodes) = answer.get("node_errors").and_then(Value::as_object) {
        for node in nodes.values() {
            for err in node
                .get("errors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let kind = err.get("type").and_then(Value::as_str).unwrap_or_default();
                let detail = err
                    .get("details")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if kind == "value_not_in_list" {
                    if detail.contains("ckpt_name") {
                        return GenError::MissingModel {
                            name: plan.model.clone(),
                        };
                    }
                    if let Some((lora, _)) = &plan.lora
                        && detail.contains("lora_name")
                    {
                        return GenError::MissingModel { name: lora.clone() };
                    }
                }
                details.push(detail.to_string());
            }
        }
    }
    let message = answer
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("the server refused the request");
    let message = std::iter::once(message.to_string())
        .chain(details.into_iter().filter(|d| !d.is_empty()))
        .collect::<Vec<_>>()
        .join(": ");
    GenError::BadParameters { message }
}

/// An execution error: out of GPU memory, or the server's message.
fn failed(kind: &str, message: &str) -> GenError {
    let lower = message.to_ascii_lowercase();
    if kind.contains("OutOfMemory")
        || lower.contains("out of memory")
        || lower.contains("allocation on device")
    {
        GenError::OutOfMemory
    } else {
        GenError::Failed {
            message: message.trim().to_string(),
        }
    }
}

/// The socket event `text` as it concerns `prompt_id`.
fn event(text: &str, prompt_id: &str) -> Event {
    let Ok(msg) = serde_json::from_str::<Value>(text) else {
        return Event::Other;
    };
    let data = msg.get("data").cloned().unwrap_or(Value::Null);
    if data.get("prompt_id").and_then(Value::as_str) != Some(prompt_id) {
        return Event::Other;
    }
    let text_of = |key: &str| {
        data.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match msg.get("type").and_then(Value::as_str) {
        Some("execution_start") => Event::Started,
        Some("progress") => Event::Progress {
            node: text_of("node"),
            value: data
                .get("value")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(0),
        },
        Some("executing") if data.get("node").is_none_or(Value::is_null) => Event::Done,
        Some("execution_success") => Event::Done,
        Some("execution_error") => Event::Error {
            kind: text_of("exception_type"),
            message: text_of("exception_message"),
        },
        Some("execution_interrupted") => Event::Interrupted,
        _ => Event::Other,
    }
}

/// A binary preview frame as a data URL: type 1 (image type + bytes) or
/// type 4 (metadata + bytes).
fn preview(frame: &[u8]) -> Option<String> {
    let word = |at: usize| -> Option<u32> {
        frame
            .get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
    };
    let (mime, image) = match word(0)? {
        1 => {
            let mime = if word(4)? == 2 {
                "image/png"
            } else {
                "image/jpeg"
            };
            (mime.to_string(), frame.get(8..)?)
        }
        4 => {
            let len = usize::try_from(word(4)?).ok()?;
            let meta: Value = serde_json::from_slice(frame.get(8..8 + len)?).ok()?;
            let mime = meta
                .get("image_type")
                .and_then(Value::as_str)
                .unwrap_or("image/jpeg")
                .to_string();
            (mime, frame.get(8 + len..)?)
        }
        _ => return None,
    };
    if image.is_empty() {
        return None;
    }
    Some(format!(
        "data:{mime};base64,{}",
        pincel_agent::base64::encode(image)
    ))
}

/// Where `prompt_id` is in `GET /queue`.
fn place(url: &Url, prompt_id: &str, cfg: &GenConfig) -> Result<Place, GenError> {
    let queue = get_json(url, "/queue", cfg)?;
    let entries = |key: &str| -> Vec<(f64, String)> {
        queue
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|e| Some((e.get(0)?.as_f64()?, e.get(1)?.as_str()?.to_string())))
            .collect()
    };
    let running = entries("queue_running");
    if running.iter().any(|(_, id)| id == prompt_id) {
        return Ok(Place::Running);
    }
    let pending = entries("queue_pending");
    let Some((number, _)) = pending.iter().find(|(_, id)| id == prompt_id) else {
        return Ok(Place::Gone);
    };
    let before = pending.iter().filter(|(n, _)| n < number).count() + running.len();
    Ok(Place::Pending {
        ahead: u32::try_from(before).unwrap_or(u32::MAX),
    })
}

/// Removes `prompt_id` from the queue, or interrupts it if it runs —
/// never another job (AC-09).
fn cancel(url: &Url, prompt_id: &str, cfg: &GenConfig) {
    let interrupt = || {
        let _ = post_json(url, "/interrupt", &json!({ "prompt_id": prompt_id }), cfg);
    };
    match place(url, prompt_id, cfg) {
        Ok(Place::Running) => interrupt(),
        Ok(Place::Pending { .. }) => {
            let _ = post_json(url, "/queue", &json!({ "delete": [prompt_id] }), cfg);
            // It may have started in between.
            if place(url, prompt_id, cfg) == Ok(Place::Running) {
                interrupt();
            }
        }
        Ok(Place::Gone) | Err(_) => {}
    }
}

/// Percent-encodes a query value.
fn query(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The history entry of `prompt_id`, waiting up to the request timeout
/// for the server to write it (it does so after the last socket event).
fn history(url: &Url, prompt_id: &str, cfg: &GenConfig) -> Result<Value, GenError> {
    let until = Instant::now() + cfg.request;
    loop {
        let all = get_json(url, &format!("/history/{}", query(prompt_id)), cfg)?;
        if let Some(entry) = all.get(prompt_id) {
            return Ok(entry.clone());
        }
        if Instant::now() >= until {
            return Err(GenError::BadAnswer);
        }
        std::thread::sleep(cfg.poll);
    }
}

/// The variants of a finished prompt, in order, and why some are missing.
fn results(plan: &Plan, prompt_id: &str, cfg: &GenConfig) -> Result<Outcome, GenError> {
    let entry = history(&plan.url, prompt_id, cfg)?;
    let mut error = entry
        .pointer("/status/messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|m| m.get(0).and_then(Value::as_str) == Some("execution_error"))
        .map(|m| {
            let field = |k: &str| {
                m.get(1)
                    .and_then(|d| d.get(k))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            failed(&field("exception_type"), &field("exception_message"))
        });
    let mut variants = Vec::new();
    for i in 0..plan.count {
        let Some(image) = entry
            .pointer(&format!("/outputs/{}/images/0", workflow::save_id(i)))
            .cloned()
        else {
            continue;
        };
        let field = |k: &str| image.get(k).and_then(Value::as_str).unwrap_or_default();
        let path = format!(
            "/view?filename={}&subfolder={}&type={}",
            query(field("filename")),
            query(field("subfolder")),
            query(field("type"))
        );
        match http::get(&plan.url, &path, cfg.request) {
            Ok(res) if res.status == 200 && !res.body.is_empty() => variants.push(res.body),
            Ok(_) => {}
            Err(_) => {
                error.get_or_insert(GenError::Unreachable);
                break;
            }
        }
    }
    let got = u32::try_from(variants.len()).unwrap_or(u32::MAX);
    if got < plan.count && error.is_none() {
        error = Some(GenError::Fewer {
            got,
            wanted: plan.count,
        });
    }
    if variants.is_empty() {
        return Err(error.unwrap_or(GenError::Fewer {
            got: 0,
            wanted: plan.count,
        }));
    }
    Ok(Outcome::Done(variants, error))
}

/// A client id no other job shares.
fn client_id(job: u64) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!("pincel-{}-{job}-{nanos:08x}", std::process::id())
}

/// Runs job `job` to its end.
pub fn run(
    plan: &Plan,
    job: u64,
    cancelled: &AtomicBool,
    cfg: &GenConfig,
    emit: &dyn Fn(GenEvent),
) -> Result<Outcome, GenError> {
    let url = &plan.url;
    let cid = client_id(job);
    // Connected before the prompt is queued, so no event is missed.
    let mut socket = Socket::connect(url, &cid, cfg.connect, cfg.poll);
    let input = plan
        .input
        .as_deref()
        .map(|png| upload(url, &format!("{cid}-input.png"), png, cfg))
        .transpose()?;
    let mask = plan
        .mask
        .as_deref()
        .map(|png| upload(url, &format!("{cid}-mask.png"), png, cfg))
        .transpose()?;
    if cancelled.load(Ordering::SeqCst) {
        return Ok(Outcome::Cancelled);
    }
    let prompt = workflow::build(&Template {
        model: &plan.model,
        family: plan.family,
        lora: plan.lora.as_ref().map(|(n, s)| (n.as_str(), *s)),
        prompt: &plan.prompt,
        negative: &plan.negative,
        seed: plan.seed,
        count: plan.count,
        strength: plan.strength,
        width: plan.width,
        height: plan.height,
        input: input.as_deref(),
        mask: mask.as_deref(),
    });
    let res = post_json(
        url,
        "/prompt",
        &json!({ "prompt": prompt, "client_id": cid }),
        cfg,
    )?;
    if res.status != 200 {
        return Err(refused(plan, &res));
    }
    let answer: Value = serde_json::from_slice(&res.body).map_err(|_| GenError::BadAnswer)?;
    let prompt_id = answer
        .get("prompt_id")
        .and_then(Value::as_str)
        .ok_or(GenError::BadAnswer)?
        .to_string();
    let steps = workflow::preset(plan.family).steps;
    let total = steps * plan.count;
    let mut running = false;
    let mut ahead = None;
    let mut next_check = Instant::now();
    loop {
        if cancelled.load(Ordering::SeqCst) {
            cancel(url, &prompt_id, cfg);
            return Ok(Outcome::Cancelled);
        }
        match socket.as_mut().map(Socket::next) {
            Some(Ok(Some(Frame::Text(text)))) => match event(&text, &prompt_id) {
                Event::Started if !running => {
                    running = true;
                    emit(GenEvent::Running { job });
                }
                Event::Progress { node, value } => {
                    if !running {
                        running = true;
                        emit(GenEvent::Running { job });
                    }
                    let done = workflow::variant_of(&node).map_or(0, |i| i * steps);
                    emit(GenEvent::Progress {
                        job,
                        step: (done + value).min(total),
                        total,
                    });
                }
                Event::Done => break,
                Event::Error { kind, message } => {
                    // Variants saved before the error stay usable (AC-15).
                    return match results(plan, &prompt_id, cfg) {
                        Ok(Outcome::Done(v, _)) => {
                            Ok(Outcome::Done(v, Some(failed(&kind, &message))))
                        }
                        _ => Err(failed(&kind, &message)),
                    };
                }
                Event::Interrupted => {
                    return Err(GenError::Failed {
                        message: "the generation was interrupted on the server".to_string(),
                    });
                }
                Event::Started | Event::Other => {}
            },
            Some(Ok(Some(Frame::Binary(bytes)))) => {
                if running && let Some(image) = preview(&bytes) {
                    emit(GenEvent::Preview { job, image });
                }
            }
            Some(Ok(None)) => {}
            Some(Err(_)) => socket = None,
            None => std::thread::sleep(cfg.poll),
        }
        if (!running || socket.is_none()) && Instant::now() >= next_check {
            next_check = Instant::now() + cfg.queue_every;
            match place(url, &prompt_id, cfg)? {
                Place::Pending { ahead: n } => {
                    if ahead != Some(n) {
                        ahead = Some(n);
                        emit(GenEvent::Queued { job, ahead: n });
                    }
                }
                Place::Running => {
                    if !running {
                        running = true;
                        emit(GenEvent::Running { job });
                    }
                }
                Place::Gone => break,
            }
        }
    }
    results(plan, &prompt_id, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_reads_only_this_prompts_events() {
        let ev = |t: &str| event(t, "p1");
        assert_eq!(
            ev(r#"{"type":"execution_start","data":{"prompt_id":"p1"}}"#),
            Event::Started
        );
        assert_eq!(
            ev(
                r#"{"type":"progress","data":{"prompt_id":"p1","node":"sample1","value":3,"max":30}}"#
            ),
            Event::Progress {
                node: "sample1".into(),
                value: 3
            }
        );
        assert_eq!(
            ev(r#"{"type":"executing","data":{"prompt_id":"p1","node":null}}"#),
            Event::Done
        );
        assert_eq!(
            ev(r#"{"type":"executing","data":{"prompt_id":"p1","node":"save0"}}"#),
            Event::Other
        );
        assert_eq!(
            ev(r#"{"type":"progress","data":{"prompt_id":"p2","node":"sample0","value":3}}"#),
            Event::Other
        );
        assert_eq!(
            ev(r#"{"type":"status","data":{"status":{"exec_info":{"queue_remaining":3}}}}"#),
            Event::Other
        );
    }

    #[test]
    fn failed_recognises_out_of_memory() {
        assert_eq!(
            failed(
                "torch.OutOfMemoryError",
                "CUDA out of memory. Tried to allocate"
            ),
            GenError::OutOfMemory
        );
        assert_eq!(
            failed(
                "RuntimeError",
                "Allocation on device 0 would exceed allowed memory"
            ),
            GenError::OutOfMemory
        );
        assert_eq!(
            failed("ValueError", " bad shape "),
            GenError::Failed {
                message: "bad shape".into()
            }
        );
    }

    #[test]
    fn preview_reads_both_frame_types() {
        let mut v1 = vec![0, 0, 0, 1, 0, 0, 0, 1];
        v1.extend_from_slice(b"jpg");
        assert_eq!(preview(&v1).as_deref(), Some("data:image/jpeg;base64,anBn"));
        let meta = br#"{"image_type":"image/png","prompt_id":"p1"}"#;
        let mut v4 = vec![0, 0, 0, 4];
        v4.extend_from_slice(&u32::try_from(meta.len()).unwrap().to_be_bytes());
        v4.extend_from_slice(meta);
        v4.extend_from_slice(b"png");
        assert_eq!(preview(&v4).as_deref(), Some("data:image/png;base64,cG5n"));
        assert_eq!(preview(&[0, 0, 0, 2, 1]), None);
    }

    #[test]
    fn query_encodes_reserved_characters() {
        assert_eq!(query("a b&c/ä.png"), "a%20b%26c%2F%C3%A4.png");
    }
}
