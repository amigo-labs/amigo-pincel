//! The pins table of the managed install (docs/specs/comfyui-install.md,
//! Technical notes "Pins"): everything the install fetches, by exact
//! version, URL and SHA-256. Python packages are pinned by the hashed lock
//! files in `src-tauri/comfyui/locks/`; uv checks the Python build it
//! fetches against its own checksums.

use serde::Serialize;

use super::download::Pinned;
use super::hardware::Backend;

const GIB: u64 = 1024 * 1024 * 1024;

/// Room for extraction and uv's cache on top of the downloads (AC-03).
pub const RESERVE: u64 = 5 * GIB;

/// What one install of ComfyUI fetches, for one backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pins {
    /// Names the pin set in `.version`; a different one offers "Update
    /// ComfyUI" (AC-12).
    pub id: String,
    pub backend: Backend,
    pub uv_version: String,
    pub uv: Pinned,
    /// The `uv` binary's path inside its archive.
    pub uv_binary: String,
    /// The exact Python version `uv venv` installs.
    pub python: String,
    pub comfyui_version: String,
    pub comfyui: Pinned,
    /// The top-level folder of ComfyUI's archive.
    pub comfyui_dir: String,
    /// The hashed lock file `uv pip sync --require-hashes` installs.
    pub lock: String,
    /// Disk space of Python, ComfyUI and its packages once installed (AC-03).
    pub base_size: u64,
}

/// Where a model package goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelKind {
    Checkpoint,
    Lora,
}

impl ModelKind {
    /// The folder below `<install>/models`.
    pub fn folder(self) -> &'static str {
        match self {
            ModelKind::Checkpoint => "checkpoints",
            ModelKind::Lora => "loras",
        }
    }
}

/// One model package of the catalog (AC-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: String,
    /// Display name, e.g. "SDXL base" (a model name, not translated).
    pub name: String,
    pub kind: ModelKind,
    /// File name below `models/<kind>`.
    pub file: String,
    #[serde(skip)]
    pub pin: Pinned,
    /// Download size in bytes.
    pub size: u64,
    /// License name, e.g. "CreativeML Open RAIL++-M".
    pub license: String,
    pub license_url: String,
    /// Minimum VRAM (NVIDIA) in bytes.
    pub min_vram: u64,
    /// Minimum unified memory (Mac) in bytes.
    pub min_memory: u64,
    /// A package this one needs (the LoRA needs SDXL base).
    pub requires: Option<String>,
}

/// Why a package selection cannot be installed (AC-04).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionError {
    Unknown(String),
    NoCheckpoint,
    Missing { package: String, requires: String },
}

/// The packages `ids` names, in catalog order, if the selection holds: at
/// least one checkpoint, and every requirement selected too.
pub fn select(catalog: &[Package], ids: &[String]) -> Result<Vec<Package>, SelectionError> {
    if let Some(unknown) = ids.iter().find(|id| !catalog.iter().any(|p| &p.id == *id)) {
        return Err(SelectionError::Unknown(unknown.clone()));
    }
    let chosen: Vec<Package> = catalog
        .iter()
        .filter(|p| ids.contains(&p.id))
        .cloned()
        .collect();
    if !chosen.iter().any(|p| p.kind == ModelKind::Checkpoint) {
        return Err(SelectionError::NoCheckpoint);
    }
    if let Some(p) = chosen.iter().find(|p| {
        p.requires
            .as_ref()
            .is_some_and(|r| !chosen.iter().any(|q| &q.id == r))
    }) {
        return Err(SelectionError::Missing {
            package: p.id.clone(),
            requires: p.requires.clone().unwrap_or_default(),
        });
    }
    Ok(chosen)
}

/// Names the whole pin set; bump it whenever any pin below changes, so an
/// existing install offers "Update ComfyUI" (AC-12). Researched
/// 2026-11-12 with a two-week cutoff (2026-10-29).
const PIN_SET: &str = "2026-11-12";
const UV_VERSION: &str = "0.13.6";
/// The newest 3.12 uv 0.13.6 offers on all three platforms
/// (python-build-standalone 20261026).
const PYTHON: &str = "3.12.16";
const COMFYUI_VERSION: &str = "0.43.0";

/// The parts of the pins that differ between platforms.
struct Platform {
    name: &'static str,
    backend: Backend,
    uv_url: &'static str,
    uv_size: u64,
    uv_sha256: &'static str,
    uv_binary: &'static str,
    lock: &'static str,
    base_size: u64,
}

const WINDOWS: Platform = Platform {
    name: "windows-x86_64",
    backend: Backend::Cuda,
    uv_url: "https://github.com/astral-sh/uv/releases/download/0.13.6/uv-x86_64-pc-windows-msvc.zip",
    uv_size: 15_239_500,
    uv_sha256: "a2a8820d5eea32f08ef467ef3b7f66883a9b076d5737a2fc2fbb5f458d1bb50f",
    uv_binary: "uv.exe",
    lock: include_str!("../../comfyui/locks/windows-x86_64.txt"),
    // 2.5 x the locked wheels (2.78 GB) + Python + ComfyUI, rounded up.
    base_size: 7_200_000_000,
};

const LINUX: Platform = Platform {
    name: "linux-x86_64",
    backend: Backend::Cuda,
    uv_url: "https://github.com/astral-sh/uv/releases/download/0.13.6/uv-x86_64-unknown-linux-gnu.tar.gz",
    uv_size: 16_799_024,
    uv_sha256: "f29c3e94ffabd7e6f3ba521dba169a9b7ec12100bf3257087f0f24f6123f3645",
    uv_binary: "uv-x86_64-unknown-linux-gnu/uv",
    lock: include_str!("../../comfyui/locks/linux-x86_64.txt"),
    // 2.5 x the locked wheels (3.81 GB) + Python + ComfyUI, rounded up.
    base_size: 9_800_000_000,
};

const MACOS: Platform = Platform {
    name: "macos-aarch64",
    backend: Backend::Mps,
    uv_url: "https://github.com/astral-sh/uv/releases/download/0.13.6/uv-aarch64-apple-darwin.tar.gz",
    uv_size: 14_235_670,
    uv_sha256: "60fbd5c8a1ed681845eea28d9d622fcde03c51e91a43465b64a1e17db1ed3472",
    uv_binary: "uv-aarch64-apple-darwin/uv",
    lock: include_str!("../../comfyui/locks/macos-aarch64.txt"),
    // 2.5 x the locked wheels (0.84 GB) + Python + ComfyUI, rounded up.
    base_size: 2_300_000_000,
};

/// The pins for an OS (`std::env::consts::OS`) and backend; `None` where
/// the managed install does not run (AC-01).
pub fn for_platform(os: &str, backend: Backend) -> Option<Pins> {
    let platform = match (os, backend) {
        ("windows", Backend::Cuda) => WINDOWS,
        ("linux", Backend::Cuda) => LINUX,
        ("macos", Backend::Mps) => MACOS,
        _ => return None,
    };
    Some(Pins {
        id: format!("{PIN_SET}/{}", platform.name),
        backend: platform.backend,
        uv_version: UV_VERSION.to_string(),
        uv: Pinned {
            url: platform.uv_url.to_string(),
            size: platform.uv_size,
            sha256: platform.uv_sha256.to_string(),
        },
        uv_binary: platform.uv_binary.to_string(),
        python: PYTHON.to_string(),
        comfyui_version: COMFYUI_VERSION.to_string(),
        comfyui: Pinned {
            url: "https://github.com/Comfy-Org/ComfyUI/archive/refs/tags/v0.43.0.zip".to_string(),
            size: 13_490_719,
            sha256: "8abcf52bdb09e5eedbd049ecbe58943bcecf72cddfd42da586b2a2f911861fd1".to_string(),
        },
        comfyui_dir: "ComfyUI-0.43.0".to_string(),
        lock: platform.lock.to_string(),
        base_size: platform.base_size,
    })
}

/// The pins for this machine.
pub fn current(backend: Backend) -> Option<Pins> {
    for_platform(std::env::consts::OS, backend)
}

/// One catalog entry; see [`catalog`].
struct Entry {
    id: &'static str,
    name: &'static str,
    kind: ModelKind,
    file: &'static str,
    url: &'static str,
    size: u64,
    sha256: &'static str,
    license: &'static str,
    license_url: &'static str,
    min_vram_gib: u64,
    min_memory_gib: u64,
    requires: Option<&'static str>,
}

const OPENRAIL_M: &str = "https://huggingface.co/spaces/CompVis/stable-diffusion-license";

/// Every URL is pinned to a Hugging Face commit of a repository that is
/// not gated (downloadable without login); size and SHA-256 are the LFS
/// object's. Minimum memory: SD 1.5's 2.1 GB fp16 checkpoint fits 4 GB
/// VRAM; SDXL's 6.9 GB runs from 6 GB with ComfyUI's offloading; Flux.1
/// schnell's ~12 GB diffusion model sets its floor, and MPS has no fp8, so
/// a Mac casts it to 16 bit.
const ENTRIES: [Entry; 4] = [
    Entry {
        id: "sdxl",
        name: "SDXL base 1.0",
        kind: ModelKind::Checkpoint,
        file: "sd_xl_base_1.0.safetensors",
        url: "https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/462165984030d82259a11f4367a4eed129e94a7b/sd_xl_base_1.0.safetensors",
        size: 6_938_078_334,
        sha256: "31e35c80fc4829d14f90153f4c74cd59c90b779f6afe05a74cd6120b893f7e5b",
        license: "CreativeML Open RAIL++-M",
        license_url: "https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/blob/462165984030d82259a11f4367a4eed129e94a7b/LICENSE.md",
        min_vram_gib: 6,
        min_memory_gib: 16,
        requires: None,
    },
    Entry {
        id: "sd15",
        name: "SD 1.5",
        kind: ModelKind::Checkpoint,
        file: "v1-5-pruned-emaonly-fp16.safetensors",
        url: "https://huggingface.co/Comfy-Org/stable-diffusion-1.5/resolve/9cfd069101959ca3828bf9c04a4419870832b74f/v1-5-pruned-emaonly-fp16.safetensors",
        size: 2_132_696_762,
        sha256: "e9476a13728cd75d8279f6ec8bad753a66a1957ca375a1464dc63b37db6e3916",
        license: "CreativeML Open RAIL-M",
        license_url: OPENRAIL_M,
        min_vram_gib: 4,
        min_memory_gib: 8,
        requires: None,
    },
    Entry {
        id: "flux-schnell",
        name: "Flux.1 schnell",
        kind: ModelKind::Checkpoint,
        file: "flux1-schnell-fp8.safetensors",
        url: "https://huggingface.co/Comfy-Org/Flux-1-Schnell/resolve/d31e04067066d92c55ece2b01748dcc337725d7a/flux1-schnell-fp8.safetensors",
        size: 17_236_328_572,
        sha256: "ead426278b49030e9da5df862994f25ce94ab2ee4df38b556ddddb3db093bf72",
        license: "Apache-2.0",
        license_url: "https://www.apache.org/licenses/LICENSE-2.0",
        min_vram_gib: 12,
        min_memory_gib: 32,
        requires: None,
    },
    Entry {
        id: "pixel-art-xl",
        name: "Pixel Art XL (LoRA)",
        kind: ModelKind::Lora,
        file: "pixel-art-xl.safetensors",
        url: "https://huggingface.co/nerijs/pixel-art-xl/resolve/8bf4a4d9ea283e00a51fafda8e0539f8248ea037/pixel-art-xl.safetensors",
        size: 170_543_052,
        sha256: "4234637cb80c998f41e348e6a6cb6bc20d8d038b2b0f256b6129b3b5e353eef7",
        license: "CreativeML Open RAIL-M",
        license_url: OPENRAIL_M,
        min_vram_gib: 6,
        min_memory_gib: 16,
        requires: Some("sdxl"),
    },
];

/// The model packages (AC-04), in display order.
pub fn catalog() -> Vec<Package> {
    ENTRIES
        .iter()
        .map(|e| Package {
            id: e.id.to_string(),
            name: e.name.to_string(),
            kind: e.kind,
            file: e.file.to_string(),
            pin: Pinned {
                url: e.url.to_string(),
                size: e.size,
                sha256: e.sha256.to_string(),
            },
            size: e.size,
            license: e.license.to_string(),
            license_url: e.license_url.to_string(),
            min_vram: e.min_vram_gib * GIB,
            min_memory: e.min_memory_gib * GIB,
            requires: e.requires.map(str::to_string),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn catalog_lists_the_four_packages_with_size_license_and_minimum() {
        let catalog = catalog();
        let names: Vec<&str> = catalog.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(names, ["sdxl", "sd15", "flux-schnell", "pixel-art-xl"]);
        for p in &catalog {
            assert!(p.size > 0 && p.size == p.pin.size, "{}", p.id);
            assert_eq!(p.pin.sha256.len(), 64, "{}", p.id);
            assert!(p.pin.sha256.chars().all(|c| c.is_ascii_hexdigit()));
            assert!(p.pin.url.starts_with("https://huggingface.co/"), "{}", p.id);
            assert!(p.pin.url.ends_with(&p.file), "{}", p.id);
            assert!(!p.license.is_empty() && p.license_url.starts_with("https://"));
            assert!(p.min_vram > 0 && p.min_memory >= p.min_vram);
        }
        assert_eq!(catalog[2].min_vram, 12 * GIB);
        assert_eq!(catalog[3].requires.as_deref(), Some("sdxl"));
        assert_eq!(catalog[3].kind, ModelKind::Lora);
    }

    #[test]
    fn for_platform_pins_windows_linux_and_apple_silicon_only() {
        let win = for_platform("windows", Backend::Cuda).unwrap();
        assert_eq!(win.uv_binary, "uv.exe");
        assert!(win.lock.contains("torch==2.14.1+cu130"));
        assert!(
            win.lock
                .contains("--find-links https://download.pytorch.org/whl/cu130/torch/")
        );
        let linux = for_platform("linux", Backend::Cuda).unwrap();
        assert!(linux.uv.url.ends_with(".tar.gz"));
        assert!(linux.lock.contains("torch==2.14.1+cu130"));
        let mac = for_platform("macos", Backend::Mps).unwrap();
        assert!(mac.lock.contains("torch==2.15.0"));
        assert_eq!(mac.backend, Backend::Mps);
        for pins in [&win, &linux, &mac] {
            assert_eq!(pins.python, "3.12.16");
            assert!(pins.id.starts_with(PIN_SET));
            assert!(pins.lock.contains("--hash=sha256:"));
            assert!(pins.comfyui.url.ends_with("v0.43.0.zip"));
            assert!(pins.base_size > 0);
        }
        assert_eq!(for_platform("macos", Backend::Cuda), None);
        assert_eq!(for_platform("freebsd", Backend::Cuda), None);
    }

    #[test]
    fn select_needs_a_checkpoint_and_sdxl_for_the_lora() {
        let catalog = catalog();
        let chosen = select(&catalog, &ids(&["pixel-art-xl", "sdxl"])).unwrap();
        // Catalog order, not request order.
        assert_eq!(chosen[0].id, "sdxl");
        assert_eq!(
            select(&catalog, &ids(&["pixel-art-xl"])),
            Err(SelectionError::NoCheckpoint)
        );
        assert_eq!(
            select(&catalog, &ids(&["sd15", "pixel-art-xl"])),
            Err(SelectionError::Missing {
                package: "pixel-art-xl".into(),
                requires: "sdxl".into()
            })
        );
        assert_eq!(select(&catalog, &[]), Err(SelectionError::NoCheckpoint));
        assert_eq!(
            select(&catalog, &ids(&["sd15", "nope"])),
            Err(SelectionError::Unknown("nope".into()))
        );
    }
}
