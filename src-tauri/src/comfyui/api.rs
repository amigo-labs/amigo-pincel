//! What Pincel reads from a ComfyUI server: `/system_stats` (identity,
//! version, first GPU) and `/object_info` (node types, checkpoints, LoRAs).
//! See docs/specs/comfyui-connect.md, Technical notes.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::http::{self, HttpError, Url};

/// Core node types the built-in workflows use (comfyui-generate).
pub const REQUIRED_NODES: [&str; 12] = [
    "CheckpointLoaderSimple",
    "LoraLoader",
    "CLIPTextEncode",
    "EmptyLatentImage",
    "KSampler",
    "VAEDecode",
    "VAEEncode",
    "VAEEncodeForInpaint",
    "LoadImage",
    "LoadImageMask",
    "SetLatentNoiseMask",
    "SaveImage",
];

/// The first GPU a server reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    /// Total memory in bytes.
    pub vram_total: u64,
}

/// A connected server's details (AC-04, AC-11, AC-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Base URL, e.g. `http://127.0.0.1:8188`.
    pub url: String,
    pub version: String,
    pub gpu: Option<Gpu>,
    pub checkpoints: Vec<String>,
    pub loras: Vec<String>,
    /// Required node types the server lacks (AC-11).
    pub missing_nodes: Vec<String>,
    /// Pincel started (or adopted) this server and ends it on quit.
    pub started: bool,
}

/// Why a URL is not a usable ComfyUI (AC-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    Unreachable,
    NotComfy,
    /// 401, 403 or 407 — a proxy or server that wants credentials.
    Refused(u16),
}

/// Timeouts of one probe.
#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    /// `/system_stats` (2 s: a local server answers in milliseconds).
    pub probe: Duration,
    /// `/object_info`, which can be large.
    pub info: Duration,
}

fn fetch_json(url: &Url, path: &str, timeout: Duration) -> Result<Value, ProbeError> {
    let res = http::get(url, path, timeout)
        .map_err(|HttpError::Unreachable(_)| ProbeError::Unreachable)?;
    match res.status {
        200 => serde_json::from_slice(&res.body).map_err(|_| ProbeError::NotComfy),
        401 | 403 | 407 => Err(ProbeError::Refused(res.status)),
        _ => Err(ProbeError::NotComfy),
    }
}

/// Whether `url` answers as ComfyUI: its version and first GPU.
pub fn identify(url: &Url, timeout: Duration) -> Result<(String, Option<Gpu>), ProbeError> {
    let stats = fetch_json(url, "/system_stats", timeout)?;
    parse_system_stats(&stats).ok_or(ProbeError::NotComfy)
}

/// The full details of the ComfyUI at `url`.
pub fn fetch_info(url: &Url, timeouts: Timeouts) -> Result<ServerInfo, ProbeError> {
    let (version, gpu) = identify(url, timeouts.probe)?;
    let info = fetch_json(url, "/object_info", timeouts.info)?;
    let (checkpoints, loras, missing_nodes) = parse_object_info(&info);
    Ok(ServerInfo {
        url: url.display(),
        version,
        gpu,
        checkpoints,
        loras,
        missing_nodes,
        started: false,
    })
}

/// Version and first GPU from `/system_stats`; `None` without
/// `system.comfyui_version` (not a ComfyUI).
pub fn parse_system_stats(stats: &Value) -> Option<(String, Option<Gpu>)> {
    let version = stats
        .pointer("/system/comfyui_version")?
        .as_str()?
        .to_string();
    let gpu = stats
        .get("devices")
        .and_then(Value::as_array)
        .and_then(|devices| devices.first())
        .map(|device| Gpu {
            name: device
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            vram_total: device
                .get("vram_total")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        });
    Some((version, gpu))
}

/// The choices of combo input `input` of node type `node`: the classic
/// `[[choices…], {…}]` or the v3 schema's `["COMBO", {"options": […]}]`.
fn combo_choices(info: &Value, node: &str, input: &str) -> Vec<String> {
    let Some(spec) = info
        .get(node)
        .and_then(|n| n.pointer("/input/required"))
        .and_then(|r| r.get(input))
    else {
        return Vec::new();
    };
    let list = match spec.get(0) {
        Some(Value::Array(list)) => Some(list),
        Some(Value::String(kind)) if kind == "COMBO" => spec
            .get(1)
            .and_then(|o| o.get("options"))
            .and_then(Value::as_array),
        _ => None,
    };
    list.map(|l| {
        l.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

/// Checkpoints, LoRAs and the missing required node types.
pub fn parse_object_info(info: &Value) -> (Vec<String>, Vec<String>, Vec<String>) {
    let checkpoints = combo_choices(info, "CheckpointLoaderSimple", "ckpt_name");
    let loras = combo_choices(info, "LoraLoader", "lora_name");
    let missing = REQUIRED_NODES
        .iter()
        .filter(|node| info.get(**node).is_none())
        .map(|node| node.to_string())
        .collect();
    (checkpoints, loras, missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comfyui::testing::{FakeHttp, Route, info_json, stats_json};

    fn timeouts() -> Timeouts {
        Timeouts {
            probe: Duration::from_secs(2),
            info: Duration::from_secs(5),
        }
    }

    #[test]
    fn fetch_info_reads_version_gpu_checkpoints_and_loras() {
        let server = FakeHttp::comfy(
            "",
            &stats_json("0.3.40"),
            &info_json(
                &["sdxl.safetensors", "sd15.ckpt"],
                &["pixel.safetensors"],
                &[],
            ),
        );
        let info = fetch_info(&Url::local(server.port), timeouts()).unwrap();
        assert_eq!(info.version, "0.3.40");
        let gpu = info.gpu.unwrap();
        assert_eq!(gpu.name, "cuda:0 NVIDIA GeForce RTX 4070 : cudaMallocAsync");
        assert_eq!(gpu.vram_total, 12_878_086_144);
        assert_eq!(info.checkpoints, ["sdxl.safetensors", "sd15.ckpt"]);
        assert_eq!(info.loras, ["pixel.safetensors"]);
        assert!(info.missing_nodes.is_empty());
        assert_eq!(info.url, format!("http://127.0.0.1:{}", server.port));
    }

    #[test]
    fn parse_object_info_names_missing_node_types() {
        let info: Value = serde_json::from_str(&info_json(
            &["a.safetensors"],
            &[],
            &["VAEEncodeForInpaint", "SetLatentNoiseMask"],
        ))
        .unwrap();
        let (_, _, missing) = parse_object_info(&info);
        assert_eq!(missing, ["VAEEncodeForInpaint", "SetLatentNoiseMask"]);
    }

    #[test]
    fn parse_object_info_reads_v3_combo_options() {
        let info = serde_json::json!({
            "CheckpointLoaderSimple": {"input": {"required": {"ckpt_name": ["COMBO", {"options": ["flux.safetensors"]}]}}},
            "LoraLoader": {"input": {"required": {"lora_name": ["COMBO", {"options": []}]}}}
        });
        let (ckpts, loras, missing) = parse_object_info(&info);
        assert_eq!(ckpts, ["flux.safetensors"]);
        assert!(loras.is_empty());
        assert_eq!(missing.len(), REQUIRED_NODES.len() - 2);
    }

    #[test]
    fn parse_system_stats_without_comfyui_version_is_none() {
        let stats = serde_json::json!({"system": {"os": "nt"}, "devices": []});
        assert_eq!(parse_system_stats(&stats), None);
        let cpu_only = serde_json::json!({"system": {"comfyui_version": "0.3.1"}, "devices": []});
        assert_eq!(
            parse_system_stats(&cpu_only),
            Some(("0.3.1".to_string(), None))
        );
    }

    #[test]
    fn identify_classifies_refused_and_foreign_servers() {
        let proxy = FakeHttp::start(vec![Route::json("/system_stats", 401, "{}")]);
        assert_eq!(
            identify(&Url::local(proxy.port), Duration::from_secs(2)),
            Err(ProbeError::Refused(401))
        );
        let other = FakeHttp::start(vec![Route::json("/system_stats", 200, "{\"ok\":true}")]);
        assert_eq!(
            identify(&Url::local(other.port), Duration::from_secs(2)),
            Err(ProbeError::NotComfy)
        );
        let html = FakeHttp::start(vec![Route::raw(
            "/system_stats",
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n<html",
        )]);
        assert_eq!(
            identify(&Url::local(html.port), Duration::from_secs(2)),
            Err(ProbeError::NotComfy)
        );
        let missing = FakeHttp::start(vec![]);
        assert_eq!(
            identify(&Url::local(missing.port), Duration::from_secs(2)),
            Err(ProbeError::NotComfy)
        );
    }
}
