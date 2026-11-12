//! Unpacking the verified `uv` and ComfyUI archives of the managed install
//! (docs/specs/comfyui-install.md, Technical notes): zip on Windows and for
//! ComfyUI's source archive, tar.gz for `uv` on Linux and macOS.

use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;

use super::download::InstallError;

/// The archive formats the pins use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Zip,
    TarGz,
}

impl Kind {
    /// The format of the file a URL names; `None` for anything else.
    pub fn of(url: &str) -> Option<Self> {
        let name = url.rsplit('/').next().unwrap_or(url).to_ascii_lowercase();
        if name.ends_with(".zip") {
            Some(Self::Zip)
        } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Some(Self::TarGz)
        } else {
            None
        }
    }
}

fn broken(e: impl std::fmt::Display) -> InstallError {
    InstallError::Failed {
        message: format!("broken archive: {e}"),
    }
}

/// `path` below `prefix`, without it; `None` for entries outside the
/// prefix, the prefix itself, and anything that would leave `dest`.
fn relative(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(prefix).ok()?;
    let safe = rest.components().all(|c| matches!(c, Component::Normal(_)));
    (safe && rest.components().next().is_some()).then(|| rest.to_path_buf())
}

/// Extracts the entries of `archive` below the folder `prefix` (empty for
/// all) into `dest`, without the prefix. Links are skipped. Returns the
/// number of files written.
pub fn extract(
    archive: &Path,
    kind: Kind,
    prefix: &str,
    dest: &Path,
) -> Result<usize, InstallError> {
    let prefix = Path::new(prefix);
    fs::create_dir_all(dest).map_err(|e| InstallError::io(&e))?;
    let file = File::open(archive).map_err(|e| InstallError::io(&e))?;
    match kind {
        Kind::Zip => extract_zip(file, prefix, dest),
        Kind::TarGz => extract_tar_gz(file, prefix, dest),
    }
}

fn extract_zip(file: File, prefix: &Path, dest: &Path) -> Result<usize, InstallError> {
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(broken)?;
    let mut written = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(broken)?;
        let Some(rel) = entry
            .enclosed_name()
            .and_then(|name| relative(&name, prefix))
        else {
            continue;
        };
        let target = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| InstallError::io(&e))?;
            continue;
        }
        if entry.is_symlink() {
            continue;
        }
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).map_err(|e| InstallError::io(&e))?;
        }
        let mut out = File::create(&target).map_err(|e| InstallError::io(&e))?;
        io::copy(&mut entry, &mut out).map_err(|e| InstallError::io(&e))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|e| InstallError::io(&e))?;
        }
        written += 1;
    }
    Ok(written)
}

fn extract_tar_gz(file: File, prefix: &Path, dest: &Path) -> Result<usize, InstallError> {
    let mut tar = tar::Archive::new(GzDecoder::new(BufReader::new(file)));
    let mut written = 0;
    for entry in tar.entries().map_err(broken)? {
        let mut entry = entry.map_err(broken)?;
        let path = entry.path().map_err(broken)?.into_owned();
        let Some(rel) = relative(&path, prefix) else {
            continue;
        };
        let target = dest.join(rel);
        match entry.header().entry_type() {
            tar::EntryType::Directory => {
                fs::create_dir_all(&target).map_err(|e| InstallError::io(&e))?;
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => {
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir).map_err(|e| InstallError::io(&e))?;
                }
                // Keeps the mode bits: uv must stay executable.
                entry.unpack(&target).map_err(|e| InstallError::io(&e))?;
                written += 1;
            }
            _ => {}
        }
    }
    Ok(written)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    /// A zip of `(path, bytes)` entries (stored, like a source archive).
    pub fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        for (path, bytes) in entries {
            zip.start_file(*path, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        out.into_inner()
    }

    /// A tar.gz of `(path, bytes)` entries, executable.
    pub fn tar_gz_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (path, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, path, *bytes).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn kind_of_reads_the_file_extension() {
        assert_eq!(
            Kind::of("https://x/uv-x86_64-pc-windows-msvc.zip"),
            Some(Kind::Zip)
        );
        assert_eq!(
            Kind::of("https://x/uv-aarch64-apple-darwin.tar.gz"),
            Some(Kind::TarGz)
        );
        assert_eq!(Kind::of("https://x/model.safetensors"), None);
    }

    #[test]
    fn extract_zip_strips_the_prefix_and_skips_other_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("c.zip");
        fs::write(
            &archive,
            zip_bytes(&[
                ("ComfyUI-0.3.99/main.py", b"print(1)"),
                ("ComfyUI-0.3.99/comfy/sd.py", b"x"),
                ("other/readme.txt", b"no"),
            ]),
        )
        .unwrap();
        let dest = tmp.path().join("Pincel ComfyUI – ü").join("ComfyUI");
        assert_eq!(
            extract(&archive, Kind::Zip, "ComfyUI-0.3.99", &dest).unwrap(),
            2
        );
        assert_eq!(fs::read(dest.join("main.py")).unwrap(), b"print(1)");
        assert!(dest.join("comfy").join("sd.py").is_file());
        assert!(!dest.join("other").exists());
    }

    #[test]
    fn extract_tar_gz_keeps_files_below_the_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tmp.path().join("uv.tar.gz");
        fs::write(
            &archive,
            tar_gz_bytes(&[
                ("uv-aarch64-apple-darwin/uv", b"#!/bin/sh\n"),
                ("uv-aarch64-apple-darwin/uvx", b"#!/bin/sh\n"),
            ]),
        )
        .unwrap();
        let dest = tmp.path().join("uv");
        assert_eq!(
            extract(&archive, Kind::TarGz, "uv-aarch64-apple-darwin", &dest).unwrap(),
            2
        );
        assert!(dest.join("uv").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dest.join("uv")).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "uv stays executable");
        }
    }

    #[test]
    fn relative_refuses_paths_that_leave_the_destination() {
        assert_eq!(relative(Path::new("a/../../x"), Path::new("")), None);
        assert_eq!(relative(Path::new("p"), Path::new("p")), None);
        assert_eq!(
            relative(Path::new("p/a/b"), Path::new("p")),
            Some(PathBuf::from("a").join("b"))
        );
    }
}
