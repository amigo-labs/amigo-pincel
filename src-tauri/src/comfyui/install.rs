//! Recognising a ComfyUI installation Pincel can start (AC-05, AC-13).

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A folder Pincel can start ComfyUI from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    /// The folder the user picked.
    pub root: PathBuf,
    /// The interpreter that runs `main.py`.
    pub python: PathBuf,
    /// The folder holding `main.py`.
    pub comfy_dir: PathBuf,
}

/// What keeps a folder from being a startable installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderProblem {
    /// The folder does not exist (any more).
    NotFound,
    /// No `main.py`, neither in the folder nor in its `ComfyUI` subfolder.
    NoComfyUi,
    /// ComfyUI is there, but no Python environment to run it.
    NoPython,
    /// ComfyUI Desktop (the Electron app) or its data folder.
    Desktop,
}

/// Classifies `dir`: a portable build, a folder with its own Python
/// environment, or a folder whose `ComfyUI` subfolder is one.
pub fn detect(dir: &Path) -> Result<Installation, FolderProblem> {
    if !dir.is_dir() {
        return Err(FolderProblem::NotFound);
    }
    if is_desktop(dir) {
        return Err(FolderProblem::Desktop);
    }
    let found = |python: PathBuf, comfy_dir: PathBuf| Installation {
        root: dir.to_path_buf(),
        python,
        comfy_dir,
    };
    let sub = dir.join("ComfyUI");
    // The portable build: an embedded Python next to the ComfyUI folder.
    let embedded = dir.join("python_embeded").join("python.exe");
    if embedded.is_file() && sub.join("main.py").is_file() {
        return Ok(found(embedded, sub));
    }
    if dir.join("main.py").is_file() {
        return venv_python(dir)
            .map(|python| found(python, dir.to_path_buf()))
            .ok_or(FolderProblem::NoPython);
    }
    if sub.join("main.py").is_file() {
        return venv_python(&sub)
            .or_else(|| venv_python(dir))
            .map(|python| found(python, sub))
            .ok_or(FolderProblem::NoPython);
    }
    Err(FolderProblem::NoComfyUi)
}

/// The interpreter of a `venv` or `.venv` in `dir`.
fn venv_python(dir: &Path) -> Option<PathBuf> {
    let candidates: &[&[&str]] = if cfg!(windows) {
        &[&["Scripts", "python.exe"]]
    } else {
        &[&["bin", "python3"], &["bin", "python"]]
    };
    ["venv", ".venv"].iter().find_map(|venv| {
        candidates.iter().find_map(|parts| {
            let path = parts.iter().fold(dir.join(venv), |p, part| p.join(part));
            path.is_file().then_some(path)
        })
    })
}

/// ComfyUI Desktop: the Electron app's folder, or its data folder (a
/// `.venv` and models, but the app keeps `main.py` in its own resources).
fn is_desktop(dir: &Path) -> bool {
    let app = [
        "resources/app.asar",
        "Contents/Resources/app.asar",
        "ComfyUI.exe",
    ]
    .iter()
    .any(|p| dir.join(p).exists());
    let data = dir.join(".venv").is_dir()
        && dir.join("models").is_dir()
        && !dir.join("main.py").exists()
        && !dir.join("ComfyUI").join("main.py").exists();
    app || data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    fn venv_python(venv: &Path) -> PathBuf {
        if cfg!(windows) {
            venv.join("Scripts").join("python.exe")
        } else {
            venv.join("bin").join("python")
        }
    }

    #[test]
    fn detect_accepts_the_portable_build() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("python_embeded").join("python.exe"));
        touch(&root.join("ComfyUI").join("main.py"));
        let inst = detect(root).unwrap();
        assert_eq!(inst.python, root.join("python_embeded").join("python.exe"));
        assert_eq!(inst.comfy_dir, root.join("ComfyUI"));
        assert_eq!(inst.root, root);
    }

    #[test]
    fn detect_accepts_a_folder_with_its_own_venv() {
        for venv in ["venv", ".venv"] {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            touch(&root.join("main.py"));
            touch(&venv_python(&root.join(venv)));
            let inst = detect(root).unwrap();
            assert_eq!(inst.python, venv_python(&root.join(venv)));
            assert_eq!(inst.comfy_dir, root);
        }
    }

    #[test]
    fn detect_accepts_a_comfyui_subfolder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("ComfyUI").join("main.py"));
        touch(&venv_python(&root.join("ComfyUI").join(".venv")));
        assert_eq!(detect(root).unwrap().comfy_dir, root.join("ComfyUI"));
        // An environment next to the subfolder (krita-ai-diffusion's layout).
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("ComfyUI").join("main.py"));
        touch(&venv_python(&root.join("venv")));
        let inst = detect(root).unwrap();
        assert_eq!(inst.comfy_dir, root.join("ComfyUI"));
        assert_eq!(inst.python, venv_python(&root.join("venv")));
    }

    #[test]
    fn detect_says_what_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(detect(tmp.path()), Err(FolderProblem::NoComfyUi));
        touch(&tmp.path().join("ComfyUI").join("main.py"));
        assert_eq!(detect(tmp.path()), Err(FolderProblem::NoPython));
        assert_eq!(
            detect(&tmp.path().join("gone")),
            Err(FolderProblem::NotFound)
        );
    }

    #[test]
    fn detect_refuses_comfyui_desktop() {
        // The app's install folder.
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join("resources").join("app.asar"));
        touch(&tmp.path().join("resources").join("ComfyUI").join("main.py"));
        assert_eq!(detect(tmp.path()), Err(FolderProblem::Desktop));
        // Its data folder: an environment and models, but no main.py.
        let tmp = tempfile::tempdir().unwrap();
        touch(&venv_python(&tmp.path().join(".venv")));
        std::fs::create_dir_all(tmp.path().join("models")).unwrap();
        assert_eq!(detect(tmp.path()), Err(FolderProblem::Desktop));
    }
}
