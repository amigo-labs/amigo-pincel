//! What the install wizard checks before installing (docs/specs/comfyui-install.md
//! AC-01 – AC-03): the GPU or Mac chip, the free disk space and whether a
//! location is writable. Processes start through `std::process` with
//! arguments, never a shell; no new crate.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

/// Below this VRAM an NVIDIA GPU gets the "slow" warning (AC-02).
pub const SLOW_VRAM: u64 = 6 * GIB;
/// Below this unified memory a Mac gets the "slow" warning (AC-02).
pub const SLOW_MEMORY: u64 = 16 * GIB;

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// The operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Os {
    Windows,
    Linux,
    Macos,
    Other,
}

/// The PyTorch build a supported device runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Backend {
    /// NVIDIA on Windows / Linux.
    Cuda,
    /// Apple Silicon.
    Mps,
}

/// The device generation runs on: an NVIDIA GPU or a Mac chip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub name: String,
    /// VRAM (NVIDIA) or unified memory (Mac), in bytes.
    pub memory: u64,
    pub backend: Backend,
}

/// Why the managed install does not run on this system (AC-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Unsupported {
    /// A GPU that is not NVIDIA (Windows names it).
    OtherGpu { name: String },
    /// `nvidia-smi` is missing or fails: no NVIDIA GPU or no driver.
    NoNvidiaDriver,
    /// A Mac with an Intel processor.
    IntelMac,
    /// ARM Windows or Linux.
    ArmCpu,
    /// Neither Windows, Linux nor macOS.
    OtherOs,
}

/// The wizard's hardware check (AC-01, AC-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub os: Os,
    /// The first supported GPU or the Mac chip.
    pub device: Option<Device>,
    /// Set when the install cannot run here.
    pub unsupported: Option<Unsupported>,
    /// Below [`SLOW_VRAM`] / [`SLOW_MEMORY`]: generation will be slow or fail.
    pub slow: bool,
}

impl Hardware {
    /// The PyTorch build to install, when the system is supported.
    pub fn backend(&self) -> Option<Backend> {
        match (&self.unsupported, &self.device) {
            (None, Some(device)) => Some(device.backend),
            _ => None,
        }
    }
}

/// What the probes found; [`classify`] turns it into a [`Hardware`].
#[derive(Debug, Clone, Default)]
pub struct Probe {
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// Output of `nvidia-smi --query-gpu=name,memory.total
    /// --format=csv,noheader,nounits`; `None` when it is missing or fails.
    pub nvidia: Option<String>,
    /// Names of all display adapters (Windows only).
    pub adapters: Vec<String>,
    /// `sysctl -n machdep.cpu.brand_string` (macOS).
    pub mac_chip: Option<String>,
    /// `sysctl -n hw.memsize` (macOS).
    pub mac_memory: Option<u64>,
}

/// Runs the probes for this machine.
pub fn probe() -> Probe {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let mut probe = Probe {
        os,
        arch,
        ..Probe::default()
    };
    match probe.os.as_str() {
        "windows" | "linux" => {
            probe.nvidia = output(
                "nvidia-smi",
                &[
                    "--query-gpu=name,memory.total",
                    "--format=csv,noheader,nounits",
                ],
                &[],
            );
            if probe.os == "windows" && probe.nvidia.is_none() {
                probe.adapters = output(
                    "powershell",
                    &[
                        "-NoProfile",
                        "-NonInteractive",
                        "-Command",
                        "(Get-CimInstance Win32_VideoController).Name",
                    ],
                    &[],
                )
                .map(|text| lines(&text))
                .unwrap_or_default();
            }
        }
        "macos" => {
            probe.mac_chip = output("sysctl", &["-n", "machdep.cpu.brand_string"], &[])
                .map(|s| s.trim().to_string());
            probe.mac_memory = output("sysctl", &["-n", "hw.memsize"], &[])
                .and_then(|s| s.trim().parse::<u64>().ok());
        }
        _ => {}
    }
    probe
}

/// The hardware check for this machine (AC-01).
pub fn detect() -> Hardware {
    classify(&probe())
}

/// Decides what is supported (AC-01) and what is slow (AC-02).
pub fn classify(probe: &Probe) -> Hardware {
    let os = match probe.os.as_str() {
        "windows" => Os::Windows,
        "linux" => Os::Linux,
        "macos" => Os::Macos,
        _ => Os::Other,
    };
    let unsupported = |reason: Unsupported, device: Option<Device>| Hardware {
        os,
        device,
        unsupported: Some(reason),
        slow: false,
    };
    let supported = |device: Device| {
        let limit = match device.backend {
            Backend::Cuda => SLOW_VRAM,
            Backend::Mps => SLOW_MEMORY,
        };
        Hardware {
            os,
            slow: device.memory < limit,
            device: Some(device),
            unsupported: None,
        }
    };
    match os {
        Os::Other => unsupported(Unsupported::OtherOs, None),
        Os::Windows | Os::Linux if probe.arch != "x86_64" => unsupported(Unsupported::ArmCpu, None),
        Os::Windows | Os::Linux => match probe.nvidia.as_deref().and_then(parse_nvidia_smi) {
            Some(device) => supported(device),
            // An NVIDIA adapter without a working nvidia-smi is a missing driver.
            None if probe.adapters.iter().any(|n| is_nvidia(n)) => {
                unsupported(Unsupported::NoNvidiaDriver, None)
            }
            None => match probe.adapters.first() {
                Some(name) => unsupported(Unsupported::OtherGpu { name: name.clone() }, None),
                None => unsupported(Unsupported::NoNvidiaDriver, None),
            },
        },
        Os::Macos => {
            let chip = probe.mac_chip.clone().unwrap_or_default();
            // An x86-64 build under Rosetta still sees the Apple chip here.
            if !chip.starts_with("Apple") {
                return unsupported(Unsupported::IntelMac, None);
            }
            supported(Device {
                name: chip,
                memory: probe.mac_memory.unwrap_or(0),
                backend: Backend::Mps,
            })
        }
    }
}

fn is_nvidia(name: &str) -> bool {
    name.to_ascii_lowercase().contains("nvidia")
}

/// The first GPU of `nvidia-smi`'s CSV answer (`name, MiB` per line).
pub fn parse_nvidia_smi(text: &str) -> Option<Device> {
    text.lines().find_map(|line| {
        let (name, mib) = line.rsplit_once(',')?;
        let mib = mib.trim().parse::<u64>().ok()?;
        let name = name.trim();
        (!name.is_empty()).then(|| Device {
            name: name.to_string(),
            memory: mib * MIB,
            backend: Backend::Cuda,
        })
    })
}

/// The deepest existing folder of `path` (the location may not exist yet).
fn existing_ancestor(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|p| p.is_dir()).map(Path::to_path_buf)
}

/// Free bytes on the volume holding `path`; `None` when unknown (a UNC
/// path on Windows, a missing tool).
pub fn free_space(path: &Path) -> Option<u64> {
    let dir = existing_ancestor(path)?;
    if cfg!(windows) {
        // The path travels in an environment variable, never in the
        // command text.
        let text = output(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "try { ([System.IO.DriveInfo]::new([System.IO.Path]::GetPathRoot($env:PINCEL_FREE_PATH))).AvailableFreeSpace } catch { '' }",
            ],
            &[("PINCEL_FREE_PATH", dir.as_os_str())],
        )?;
        text.trim().parse().ok()
    } else {
        let mut args: Vec<&std::ffi::OsStr> = vec!["-Pk".as_ref()];
        args.push(dir.as_os_str());
        let out = Command::new("df")
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        parse_df(&String::from_utf8_lossy(&out.stdout))
    }
}

/// Available bytes from `df -Pk` output (the second line's fourth field).
pub fn parse_df(text: &str) -> Option<u64> {
    let line = text.lines().nth(1)?;
    let kib: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(kib * 1024)
}

/// Whether Pincel can create files in `dir` (it must exist).
pub fn writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".pincel-write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// Stdout of a successful `program args`, without a console window.
fn output(program: &str, args: &[&str], env: &[(&str, &std::ffi::OsStr)]) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(super::process::CREATE_NO_WINDOW);
    }
    let out = command.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(nvidia: Option<&str>, adapters: &[&str]) -> Probe {
        Probe {
            os: "windows".into(),
            arch: "x86_64".into(),
            nvidia: nvidia.map(str::to_string),
            adapters: adapters.iter().map(|s| s.to_string()).collect(),
            ..Probe::default()
        }
    }

    fn mac(chip: &str, gib: u64) -> Probe {
        Probe {
            os: "macos".into(),
            arch: "aarch64".into(),
            mac_chip: Some(chip.into()),
            mac_memory: Some(gib * GIB),
            ..Probe::default()
        }
    }

    #[test]
    fn parse_nvidia_smi_reads_the_first_gpu() {
        let device =
            parse_nvidia_smi("NVIDIA GeForce RTX 4070, 12282\nNVIDIA GeForce GTX 1050, 2048\n")
                .unwrap();
        assert_eq!(device.name, "NVIDIA GeForce RTX 4070");
        assert_eq!(device.memory, 12282 * MIB);
        assert_eq!(device.backend, Backend::Cuda);
        assert_eq!(parse_nvidia_smi("No devices were found\n"), None);
        assert_eq!(parse_nvidia_smi(""), None);
    }

    #[test]
    fn classify_nvidia_on_windows_and_linux_is_supported() {
        let hw = classify(&win(Some("NVIDIA GeForce RTX 4070, 12282"), &[]));
        assert_eq!(hw.os, Os::Windows);
        assert_eq!(hw.unsupported, None);
        assert_eq!(hw.backend(), Some(Backend::Cuda));
        assert!(!hw.slow);
        let linux = Probe {
            os: "linux".into(),
            ..win(Some("NVIDIA RTX A2000, 6138"), &[])
        };
        let hw = classify(&linux);
        assert_eq!(hw.os, Os::Linux);
        // 6138 MiB is just under 6 GiB (AC-02).
        assert!(hw.slow);
        assert_eq!(hw.backend(), Some(Backend::Cuda));
    }

    #[test]
    fn classify_apple_silicon_is_supported_and_slow_below_16_gib() {
        let hw = classify(&mac("Apple M2 Pro", 32));
        assert_eq!(hw.backend(), Some(Backend::Mps));
        assert_eq!(hw.device.as_ref().unwrap().name, "Apple M2 Pro");
        assert!(!hw.slow);
        assert!(classify(&mac("Apple M1", 8)).slow);
        assert!(!classify(&mac("Apple M3", 16)).slow);
    }

    #[test]
    fn classify_names_the_reason_for_unsupported_systems() {
        assert_eq!(
            classify(&mac("Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz", 16)).unsupported,
            Some(Unsupported::IntelMac)
        );
        assert_eq!(
            classify(&win(None, &["AMD Radeon RX 7900 XTX"])).unsupported,
            Some(Unsupported::OtherGpu {
                name: "AMD Radeon RX 7900 XTX".into()
            })
        );
        // An NVIDIA adapter whose nvidia-smi does not answer: no driver.
        assert_eq!(
            classify(&win(
                None,
                &["Intel(R) UHD Graphics", "NVIDIA GeForce RTX 3060"]
            ))
            .unsupported,
            Some(Unsupported::NoNvidiaDriver)
        );
        assert_eq!(
            classify(&win(None, &[])).unsupported,
            Some(Unsupported::NoNvidiaDriver)
        );
        let arm = Probe {
            arch: "aarch64".into(),
            ..win(Some("NVIDIA GeForce RTX 4070, 12282"), &[])
        };
        assert_eq!(classify(&arm).unsupported, Some(Unsupported::ArmCpu));
        let bsd = Probe {
            os: "freebsd".into(),
            ..Probe::default()
        };
        let hw = classify(&bsd);
        assert_eq!(hw.unsupported, Some(Unsupported::OtherOs));
        assert_eq!(hw.backend(), None);
    }

    #[test]
    fn parse_df_reads_the_available_column() {
        let text = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sda1       102400000  51200000  40960000      56% /\n";
        assert_eq!(parse_df(text), Some(40_960_000 * 1024));
        assert_eq!(parse_df("garbage"), None);
    }

    #[test]
    fn free_space_reports_the_volume_of_a_not_yet_existing_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let free = free_space(&tmp.path().join("Pincel ComfyUI").join("deeper"));
        assert!(free.is_some_and(|b| b > 0), "{free:?}");
    }

    #[test]
    fn writable_is_false_for_a_missing_folder() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(writable(tmp.path()));
        assert!(!writable(&tmp.path().join("missing")));
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}
