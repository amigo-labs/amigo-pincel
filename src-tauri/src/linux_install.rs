//! `pincel --install` / `--uninstall` for the Linux AppImage, after
//! amigo-blinzel's: the AppImage copies itself to
//! `~/.local/share/pincel/pincel.AppImage`, links `~/.local/bin/pincel`
//! and adds a desktop entry with the MIME types it opens and the icon.
//! Only that installed copy updates itself (`updater::supported`).
//! See docs/RELEASING.md and docs/specs/pincel.md §15, 2026-10-26.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The MIME types of `bundle.fileAssociations` in `tauri.conf.json`.
const MIME_TYPES: &[&str] = &[
    "application/x-aseprite",
    "image/openraster",
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/bmp",
    "image/gif",
    "image/tiff",
];

/// shared-mime-info knows every type above except Aseprite's.
const MIME_PACKAGE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-aseprite">
    <comment>Aseprite sprite</comment>
    <glob pattern="*.aseprite"/>
    <glob pattern="*.ase"/>
  </mime-type>
</mime-info>
"#;

const ICONS: &[(&str, &[u8])] = &[
    ("32x32", include_bytes!("../icons/32x32.png")),
    ("128x128", include_bytes!("../icons/128x128.png")),
    ("256x256", include_bytes!("../icons/128x128@2x.png")),
    ("512x512", include_bytes!("../icons/icon.png")),
];

/// Where an installation lives: real paths from the environment, or a
/// sandbox in tests.
#[derive(Debug, Clone)]
pub struct Layout {
    /// `$XDG_DATA_HOME`, else `~/.local/share`.
    data: PathBuf,
    /// `~/.local/bin`.
    bin: PathBuf,
}

impl Layout {
    /// The current user's layout; `None` without `$HOME`.
    pub fn from_env() -> Option<Self> {
        let home = PathBuf::from(std::env::var_os("HOME")?);
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        Some(Self::new(data, home.join(".local/bin")))
    }

    fn new(data: PathBuf, bin: PathBuf) -> Self {
        Self { data, bin }
    }

    /// The installed AppImage.
    pub fn appimage(&self) -> PathBuf {
        self.data.join("pincel/pincel.AppImage")
    }

    fn bin_link(&self) -> PathBuf {
        self.bin.join("pincel")
    }

    fn desktop_file(&self) -> PathBuf {
        self.data.join("applications/pincel.desktop")
    }

    fn mime_package(&self) -> PathBuf {
        self.data.join("mime/packages/pincel.xml")
    }

    fn icon(&self, size: &str) -> PathBuf {
        self.data
            .join("icons/hicolor")
            .join(size)
            .join("apps/pincel.png")
    }
}

/// Whether the running AppImage is the installed copy.
pub fn running_installed() -> bool {
    let (Some(running), Some(layout)) = (std::env::var_os("APPIMAGE"), Layout::from_env()) else {
        return false;
    };
    match (
        fs::canonicalize(running),
        fs::canonicalize(layout.appimage()),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn desktop_entry(exec: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Pincel\n\
         GenericName=Image Editor\n\
         GenericName[de]=Bildbearbeitung\n\
         Comment=Pixel-art and image editor\n\
         Comment[de]=Pixel-Art- und Bildbearbeitung\n\
         Exec=\"{}\" %f\n\
         Icon=pincel\n\
         Terminal=false\n\
         Categories=Graphics;2DGraphics;RasterGraphics;\n\
         MimeType={};\n",
        exec.display(),
        MIME_TYPES.join(";")
    )
}

fn write_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, bytes)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Install the AppImage at `source` into `layout`. Installing over the
/// installed copy (also from itself) replaces it.
pub fn install(layout: &Layout, source: &Path) -> io::Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let target = layout.appimage();
    let same = matches!(
        (fs::canonicalize(source), fs::canonicalize(&target)),
        (Ok(a), Ok(b)) if a == b
    );
    if !same {
        // Copy beside it, then rename: a running copy keeps its inode.
        let dir = target
            .parent()
            .expect("the AppImage path has a parent directory");
        fs::create_dir_all(dir)?;
        let partial = dir.join("pincel.AppImage.partial");
        fs::copy(source, &partial)?;
        fs::set_permissions(&partial, fs::Permissions::from_mode(0o755))?;
        fs::rename(&partial, &target)?;
    }

    fs::create_dir_all(&layout.bin)?;
    let link = layout.bin_link();
    remove_if_present(&link)?;
    symlink(&target, &link)?;

    write_file(&layout.desktop_file(), desktop_entry(&target).as_bytes())?;
    write_file(&layout.mime_package(), MIME_PACKAGE.as_bytes())?;
    for (size, png) in ICONS {
        write_file(&layout.icon(size), png)?;
    }
    refresh_caches(layout);
    Ok(())
}

/// Remove everything `install` created. Missing parts are fine.
pub fn uninstall(layout: &Layout) -> io::Result<()> {
    remove_if_present(&layout.bin_link())?;
    remove_if_present(&layout.desktop_file())?;
    remove_if_present(&layout.mime_package())?;
    for (size, _) in ICONS {
        remove_if_present(&layout.icon(size))?;
    }
    remove_if_present(&layout.appimage())?;
    if let Some(dir) = layout.appimage().parent() {
        let _ = fs::remove_dir(dir);
    }
    refresh_caches(layout);
    Ok(())
}

/// Best effort: desktops pick the files up without these, only later.
fn refresh_caches(layout: &Layout) {
    let runs: [(&str, PathBuf); 2] = [
        ("update-mime-database", layout.data.join("mime")),
        ("update-desktop-database", layout.data.join("applications")),
    ];
    for (tool, dir) in runs {
        if dir.is_dir() {
            let _ = Command::new(tool).arg(&dir).output();
        }
    }
}

/// Handle `--install` / `--uninstall` before Tauri starts. Returns the
/// process exit code when one of them was given (0 done, 1 failed).
pub fn run_cli(args: &[String]) -> Option<i32> {
    let installing = args.iter().any(|a| a == "--install");
    let uninstalling = args.iter().any(|a| a == "--uninstall");
    if !installing && !uninstalling {
        return None;
    }
    let Some(layout) = Layout::from_env() else {
        eprintln!("pincel: $HOME is not set");
        return Some(1);
    };
    let result = if uninstalling {
        uninstall(&layout).map(|()| "Pincel is uninstalled.".to_string())
    } else {
        match std::env::var_os("APPIMAGE") {
            Some(source) => install(&layout, Path::new(&source)).map(|()| {
                format!(
                    "Pincel is installed: {} (command: {}).",
                    layout.appimage().display(),
                    layout.bin_link().display()
                )
            }),
            None => Err(io::Error::other(
                "only the AppImage installs itself; .deb / .rpm go through the package manager",
            )),
        }
    };
    match result {
        Ok(message) => {
            println!("{message}");
            Some(0)
        }
        Err(e) => {
            eprintln!("pincel: {e}");
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> (tempfile::TempDir, Layout, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path().join("share"), dir.path().join("bin"));
        let source = dir.path().join("Downloads/pincel-linux-x64.AppImage");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"appimage v1").unwrap();
        (dir, layout, source)
    }

    #[test]
    fn install_creates_every_part() {
        let (_dir, layout, source) = sandbox();
        install(&layout, &source).unwrap();
        assert_eq!(fs::read(layout.appimage()).unwrap(), b"appimage v1");
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(layout.appimage())
                .unwrap()
                .permissions()
                .mode()
        };
        assert_eq!(mode & 0o777, 0o755);
        assert_eq!(fs::read_link(layout.bin_link()).unwrap(), layout.appimage());
        let entry = fs::read_to_string(layout.desktop_file()).unwrap();
        assert!(entry.contains(&format!("Exec=\"{}\" %f", layout.appimage().display())));
        assert!(entry.contains("MimeType=application/x-aseprite;image/openraster;"));
        assert!(layout.mime_package().is_file());
        for (size, _) in ICONS {
            assert!(layout.icon(size).is_file(), "{size}");
        }
    }

    #[test]
    fn install_again_replaces_the_copy() {
        let (_dir, layout, source) = sandbox();
        install(&layout, &source).unwrap();
        fs::write(&source, b"appimage v2").unwrap();
        install(&layout, &source).unwrap();
        assert_eq!(fs::read(layout.appimage()).unwrap(), b"appimage v2");
    }

    #[test]
    fn install_from_the_installed_copy_keeps_it() {
        let (_dir, layout, source) = sandbox();
        install(&layout, &source).unwrap();
        install(&layout, &layout.appimage()).unwrap();
        assert_eq!(fs::read(layout.appimage()).unwrap(), b"appimage v1");
    }

    #[test]
    fn uninstall_removes_every_part() {
        let (_dir, layout, source) = sandbox();
        install(&layout, &source).unwrap();
        uninstall(&layout).unwrap();
        assert!(!layout.appimage().exists());
        assert!(!layout.appimage().parent().unwrap().exists());
        assert!(fs::symlink_metadata(layout.bin_link()).is_err());
        assert!(!layout.desktop_file().exists());
        assert!(!layout.mime_package().exists());
        for (size, _) in ICONS {
            assert!(!layout.icon(size).exists(), "{size}");
        }
        // The download itself is left alone.
        assert!(source.is_file());
    }

    #[test]
    fn uninstall_without_install_is_fine() {
        let (_dir, layout, _source) = sandbox();
        uninstall(&layout).unwrap();
    }

    #[test]
    fn run_cli_ignores_other_args() {
        let args = vec!["pincel".to_string(), "/tmp/a.png".to_string()];
        assert_eq!(run_cli(&args), None);
    }

    #[test]
    fn mime_types_match_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let declared: Vec<&str> = conf["bundle"]["fileAssociations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["mimeType"].as_str().unwrap())
            .collect();
        assert_eq!(declared, MIME_TYPES);
    }
}
