// The desktop app's native job backend (docs/specs/performance-desktop-jobs.md):
// the heavy one-shot Image-mode jobs of the Web Worker backend
// (`ui/src/modes/image/jobs/protocol.ts`) — encoding, and effect previews and
// regions on a document copy — run here in native Rust, on all cores. Both
// backends call the same `pincel_image_core::jobs` functions, and the cores
// compute their float math with `libm` on both targets, so the results are
// byte-identical (AC-01). The command is a thin wrapper
// (AGENTS.md §5.5): the job comes as JSON in a request header, the document
// snapshot as the raw request body, the result goes back as raw bytes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pincel_effects::EffectSpec;
use pincel_image_core::codec::snapshot::from_snapshot_bytes;
use pincel_image_core::document::Document;
use pincel_image_core::jobs::{self, EncodeFormat};
use serde::Deserialize;

/// The request header that carries the job (the UI's `ImageJob` without
/// its snapshot, as JSON).
pub const JOB_HEADER: &str = "pincel-job";

/// One effect run on a layer (the UI's `EffectJob`).
#[derive(Debug, Clone, Deserialize)]
pub struct EffectJob {
    layer: usize,
    name: String,
    params: Vec<f64>,
}

/// A job, as the UI's `ImageJob` names it; `encode` and `load` take the
/// document snapshot from the request body.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum NativeJob {
    /// Encode the snapshot (autosave, save, export).
    Encode {
        format: String,
        level: u8,
        quality: u8,
    },
    /// Keep a copy of the snapshot under `key` for one effect session.
    Load { key: String },
    /// Free the copy under `key`.
    Drop { key: String },
    /// The composite with the effect previewed (canvas-sized RGBA8).
    Preview { key: String, effect: EffectJob },
    /// The effect's region in `jobs::encode_region`'s wire form.
    Region { key: String, effect: EffectJob },
}

/// The document copies the effect sessions loaded, by key.
#[derive(Debug, Default, Clone)]
pub struct NativeJobs {
    docs: Arc<Mutex<HashMap<String, Arc<Document>>>>,
}

impl NativeJobs {
    /// Runs `job`; `body` is the snapshot for `encode` and `load`.
    pub fn run(&self, job: NativeJob, body: &[u8]) -> Result<Vec<u8>, String> {
        let text = |e: &dyn std::fmt::Display| e.to_string();
        match job {
            NativeJob::Encode {
                format,
                level,
                quality,
            } => {
                let format = EncodeFormat::parse(&format).map_err(|e| text(&e))?;
                let doc = from_snapshot_bytes(body).map_err(|e| text(&e))?;
                jobs::encode(&doc, format, level, quality).map_err(|e| text(&e))
            }
            NativeJob::Load { key } => {
                let doc = from_snapshot_bytes(body).map_err(|e| text(&e))?;
                self.docs()?.insert(key, Arc::new(doc));
                Ok(Vec::new())
            }
            NativeJob::Drop { key } => {
                self.docs()?.remove(&key);
                Ok(Vec::new())
            }
            NativeJob::Preview { key, effect } => {
                let (doc, spec) = self.effect_input(&key, &effect)?;
                let out = jobs::preview_effect(&doc, effect.layer, &spec).map_err(|e| text(&e))?;
                Ok(out.into_raw())
            }
            NativeJob::Region { key, effect } => {
                let (doc, spec) = self.effect_input(&key, &effect)?;
                let region =
                    jobs::effect_region(&doc, effect.layer, &spec).map_err(|e| text(&e))?;
                Ok(jobs::encode_region(region))
            }
        }
    }

    fn docs(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<Document>>>, String> {
        self.docs
            .lock()
            .map_err(|_| "the native job state is poisoned".to_string())
    }

    /// The document loaded as `key` (shared, so the lock is not held while
    /// the effect runs) and the effect's spec.
    fn effect_input(
        &self,
        key: &str,
        effect: &EffectJob,
    ) -> Result<(Arc<Document>, EffectSpec), String> {
        let spec =
            EffectSpec::from_params(&effect.name, &effect.params).map_err(|e| e.to_string())?;
        let doc = self
            .docs()?
            .get(key)
            .cloned()
            .ok_or_else(|| format!("no document loaded as {key}"))?;
        Ok((doc, spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_image_core::BlendMode;
    use pincel_image_core::codec::snapshot::to_snapshot_bytes;
    use pincel_image_core::document::{ImageBuffer, Layer};
    use pincel_image_core::geometry::Rect;
    use pincel_image_core::selection::SelectionMask;
    use std::path::PathBuf;

    /// The fixtures shared with `ui/e2e/desktop-jobs.spec.ts`: the input
    /// snapshot and the job cases with the Web Worker backend's result
    /// digests (which that spec records).
    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jobs")
    }

    /// A 64×48 document with two raster layers (the top one translucent,
    /// multiplied, at 80 %) and an anti-aliased elliptical selection; ids
    /// and timestamps fixed, so its snapshot bytes are too.
    fn fixture_document() -> Document {
        let (w, h) = (64u32, 48u32);
        let pixels = |f: &dyn Fn(u32, u32) -> [u8; 4]| {
            let mut data = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    data.extend_from_slice(&f(x, y));
                }
            }
            ImageBuffer::from_raw(w, h, data).expect("fixture pixels")
        };
        let id = |n: u32| {
            serde_json::from_value(serde_json::json!(format!(
                "00000000-0000-4000-8000-{n:012}"
            )))
            .expect("fixture uuid")
        };
        let mut doc = Document::new(w, h).expect("fixture document");
        doc.id = id(1);
        doc.title = "fixture".to_string();
        doc.metadata.created_at = 0;
        doc.metadata.modified_at = 0;
        let mut base = Layer::from_pixels(
            "base",
            pixels(&|x, y| [(x * 4) as u8, (y * 5) as u8, ((x ^ y) * 8) as u8, 255]),
        );
        base.id = id(2);
        let mut paint = Layer::from_pixels(
            "paint",
            pixels(&|x, y| {
                [
                    ((x * y) % 256) as u8,
                    (200 - x) as u8,
                    (y * 3) as u8,
                    (((x + y) * 7) % 256) as u8,
                ]
            }),
        )
        .with_opacity(0.8)
        .with_blend_mode(BlendMode::Multiply);
        paint.id = id(3);
        doc.remove_layer(0).ok();
        doc.insert_layer(0, base).expect("base layer");
        doc.insert_layer(1, paint).expect("paint layer");
        while doc.layer_count() > 2 {
            doc.remove_layer(2).expect("extra layer");
        }
        doc.set_active_layer(1).expect("active layer");
        doc.selection = Some(SelectionMask::ellipse_aa(w, h, Rect::new(8, 6, 44, 34)));
        doc
    }

    /// FNV-1a, 64 bit, as lowercase hex (the e2e spec computes the same).
    fn fnv1a64(bytes: &[u8]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    #[derive(Deserialize)]
    struct Case {
        job: serde_json::Value,
        len: usize,
        fnv1a64: String,
    }

    #[derive(Deserialize)]
    struct Cases {
        cases: Vec<Case>,
    }

    #[test]
    fn fixture_snapshot_matches_its_recipe() {
        let bytes = to_snapshot_bytes(&fixture_document());
        let path = fixtures().join("doc.snapshot");
        // PINCEL_WRITE_JOB_FIXTURES=1 rewrites the fixture after a change
        // to the recipe or the snapshot format; the digests in cases.json
        // are then recorded again by ui/e2e/desktop-jobs.spec.ts.
        if std::env::var_os("PINCEL_WRITE_JOB_FIXTURES").is_some() {
            std::fs::create_dir_all(fixtures()).expect("fixture dir");
            std::fs::write(&path, &bytes).expect("write fixture");
        }
        let committed = std::fs::read(&path).expect("doc.snapshot");
        assert!(committed == bytes, "doc.snapshot is stale");
    }

    fn effect(name: &str, params: &[f64]) -> EffectJob {
        EffectJob {
            layer: 1,
            name: name.to_string(),
            params: params.to_vec(),
        }
    }

    #[test]
    fn native_job_reads_the_headers_the_ui_sends() {
        let job: NativeJob =
            serde_json::from_str(r#"{"kind":"encode","format":"png","level":6,"quality":90}"#)
                .expect("encode");
        assert!(
            matches!(job, NativeJob::Encode { ref format, level: 6, quality: 90 } if format == "png")
        );
        let job: NativeJob = serde_json::from_str(
            r#"{"kind":"region","key":"fx-1","effect":{"layer":0,"name":"invert","params":[]}}"#,
        )
        .expect("region");
        assert!(
            matches!(job, NativeJob::Region { ref key, ref effect } if key == "fx-1" && effect.name == "invert")
        );
        assert!(serde_json::from_str::<NativeJob>(r#"{"kind":"paint"}"#).is_err());
    }

    #[test]
    fn run_encodes_with_the_shared_job_function() {
        let doc = fixture_document();
        let snapshot = to_snapshot_bytes(&doc);
        let jobs = NativeJobs::default();
        for (format, level, quality) in [("ora", 6, 90), ("png", 9, 90), ("jpeg", 6, 40)] {
            let job = NativeJob::Encode {
                format: format.to_string(),
                level,
                quality,
            };
            let expected = jobs::encode(
                &doc,
                EncodeFormat::parse(format).expect("format"),
                level,
                quality,
            )
            .expect("encode");
            assert_eq!(jobs.run(job, &snapshot), Ok(expected), "{format}");
        }
        let bad = NativeJob::Encode {
            format: "psd".to_string(),
            level: 6,
            quality: 90,
        };
        assert!(jobs.run(bad, &snapshot).is_err());
        let truncated = NativeJob::Encode {
            format: "png".to_string(),
            level: 6,
            quality: 90,
        };
        assert!(jobs.run(truncated, &snapshot[..10]).is_err());
    }

    #[test]
    fn run_effect_jobs_work_on_the_loaded_copy_until_it_is_dropped() {
        let doc = fixture_document();
        let jobs = NativeJobs::default();
        let preview = || NativeJob::Preview {
            key: "fx".to_string(),
            effect: effect("gaussian_blur", &[10.0]),
        };
        assert!(jobs.run(preview(), &[]).is_err(), "nothing loaded yet");
        let load = NativeJob::Load {
            key: "fx".to_string(),
        };
        assert_eq!(jobs.run(load, &to_snapshot_bytes(&doc)), Ok(Vec::new()));

        let spec = EffectSpec::from_params("gaussian_blur", &[10.0]).expect("spec");
        let composite = jobs::preview_effect(&doc, 1, &spec).expect("preview");
        assert_eq!(jobs.run(preview(), &[]), Ok(composite.into_raw()));
        let region = NativeJob::Region {
            key: "fx".to_string(),
            effect: effect("gaussian_blur", &[10.0]),
        };
        let expected = jobs::encode_region(jobs::effect_region(&doc, 1, &spec).expect("region"));
        assert_eq!(jobs.run(region, &[]), Ok(expected));
        let unknown = NativeJob::Preview {
            key: "fx".to_string(),
            effect: effect("no_such_effect", &[]),
        };
        assert!(jobs.run(unknown, &[]).is_err());

        let drop = NativeJob::Drop {
            key: "fx".to_string(),
        };
        assert_eq!(jobs.run(drop, &[]), Ok(Vec::new()));
        assert!(jobs.run(preview(), &[]).is_err(), "the copy is gone");
    }

    #[test]
    fn run_matches_the_web_worker_backend_for_every_job_kind() {
        let snapshot = std::fs::read(fixtures().join("doc.snapshot")).expect("doc.snapshot");
        let cases: Cases = serde_json::from_slice(
            &std::fs::read(fixtures().join("cases.json")).expect("cases.json"),
        )
        .expect("cases");
        assert!(!cases.cases.is_empty());
        let jobs = NativeJobs::default();
        let mut mismatches = Vec::new();
        for case in &cases.cases {
            let job: NativeJob = serde_json::from_value(case.job.clone()).expect("job");
            let body: &[u8] = match job {
                NativeJob::Encode { .. } | NativeJob::Load { .. } => &snapshot,
                _ => &[],
            };
            match jobs.run(job, body) {
                Ok(out) if out.len() == case.len && fnv1a64(&out) == case.fnv1a64 => {}
                Ok(out) => mismatches.push(format!(
                    "{}: {} bytes {} (worker: {} bytes {})",
                    case.job,
                    out.len(),
                    fnv1a64(&out),
                    case.len,
                    case.fnv1a64
                )),
                Err(e) => mismatches.push(format!("{}: {e}", case.job)),
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }
}
