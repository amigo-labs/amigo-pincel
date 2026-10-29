//! The workspace sandbox: every path argument resolves inside one
//! directory, symlinks included (docs/specs/mcp-headless-pixel.md AC-34 …
//! AC-36).

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use pincel_agent::FileHost;

/// The error for any path that leaves the workspace.
pub const OUTSIDE: &str = "E007: path outside workspace";

/// File access confined to a workspace directory.
#[derive(Debug)]
pub struct Sandbox {
    root: PathBuf,
    read_only: bool,
}

impl Sandbox {
    /// Confine access to `root` (must exist); `read_only` refuses writes.
    pub fn new(root: &Path, read_only: bool) -> io::Result<Self> {
        Ok(Self {
            root: fs::canonicalize(root)?,
            read_only,
        })
    }

    /// The canonical workspace directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a workspace-relative `path`, refusing absolute paths, `..`
    /// that climbs out, and symlinks that lead out.
    pub fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let rel = Path::new(path);
        if path.is_empty() || rel.has_root() || rel.is_absolute() {
            return Err(OUTSIDE.to_owned());
        }
        let mut out = self.root.clone();
        for c in rel.components() {
            match c {
                Component::Normal(part) => out.push(part),
                Component::CurDir => {}
                Component::ParentDir => {
                    if out == self.root || !out.pop() {
                        return Err(OUTSIDE.to_owned());
                    }
                }
                Component::RootDir | Component::Prefix(_) => return Err(OUTSIDE.to_owned()),
            }
        }
        if !out.starts_with(&self.root) || out == self.root {
            return Err(OUTSIDE.to_owned());
        }
        // Follow symlinks through the deepest existing ancestor (the path
        // itself when it exists) and require it to stay inside.
        let mut probe = out.as_path();
        loop {
            if fs::symlink_metadata(probe).is_ok() {
                let real = fs::canonicalize(probe).map_err(|_| OUTSIDE.to_owned())?;
                if !real.starts_with(&self.root) {
                    return Err(OUTSIDE.to_owned());
                }
                break;
            }
            match probe.parent() {
                Some(p) => probe = p,
                None => return Err(OUTSIDE.to_owned()),
            }
        }
        Ok(out)
    }
}

impl Sandbox {
    /// Resolve `path` for a write, refusing it in a read-only workspace.
    fn writable(&self, path: &str) -> Result<PathBuf, String> {
        let full = self.resolve(path)?;
        if self.read_only {
            return Err("the workspace is read-only (--read-only); nothing was written".to_owned());
        }
        Ok(full)
    }
}

impl FileHost for Sandbox {
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        let full = self.resolve(path)?;
        fs::read(&full).map_err(|e| format!("{path}: {e}"))
    }

    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let full = self.writable(path)?;
        if let Some(dir) = full.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{path}: {e}"))?;
        }
        fs::write(&full, bytes).map_err(|e| format!("{path}: {e}"))
    }

    fn check_write(&mut self, path: &str) -> Result<(), String> {
        self.writable(path).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir;

    #[test]
    fn resolve_rejects_parent_escape_and_absolute_paths() {
        let dir = test_dir("sandbox-escape");
        let sb = Sandbox::new(&dir, false).expect("sandbox");
        for bad in ["../x.png", "a/../../x.png", "/etc/passwd", "", "."] {
            assert_eq!(sb.resolve(bad), Err(OUTSIDE.to_owned()), "{bad}");
        }
        #[cfg(windows)]
        assert_eq!(sb.resolve("C:\\Windows\\x.png"), Err(OUTSIDE.to_owned()));
        assert!(
            sb.resolve("a/../x.png")
                .expect("inside")
                .starts_with(sb.root())
        );
        assert!(
            sb.resolve("./out/x.png")
                .expect("inside")
                .ends_with("x.png")
        );
    }

    #[test]
    fn write_outside_writes_nothing() {
        let dir = test_dir("sandbox-write");
        let inner = dir.join("ws");
        fs::create_dir_all(&inner).expect("mkdir");
        let mut sb = Sandbox::new(&inner, false).expect("sandbox");
        assert_eq!(sb.write("../x.png", b"x"), Err(OUTSIDE.to_owned()));
        assert!(!dir.join("x.png").exists());
        sb.write("out/y.png", b"y").expect("inside write");
        assert_eq!(fs::read(inner.join("out/y.png")).expect("read"), b"y");
    }

    #[test]
    fn read_only_refuses_writes_but_reads() {
        let dir = test_dir("sandbox-ro");
        fs::write(dir.join("a.txt"), b"a").expect("seed");
        let mut sb = Sandbox::new(&dir, true).expect("sandbox");
        let e = sb.write("b.png", b"b").expect_err("read-only");
        assert!(e.contains("read-only"), "{e}");
        assert!(!dir.join("b.png").exists());
        assert_eq!(sb.read("a.txt").expect("read"), b"a");
    }

    /// `ws/link` points at a sibling `outside/` directory holding
    /// `secret.txt`; returns `(ws, outside)`.
    fn linked_workspace(name: &str, link: impl FnOnce(&Path, &Path)) -> (PathBuf, PathBuf) {
        let dir = test_dir(name);
        let ws = dir.join("ws");
        let outside = dir.join("outside");
        fs::create_dir_all(&ws).expect("mkdir");
        fs::create_dir_all(&outside).expect("mkdir");
        fs::write(outside.join("secret.txt"), b"s").expect("seed");
        link(&outside, &ws.join("link"));
        assert!(
            ws.join("link").join("secret.txt").exists(),
            "link was created"
        );
        (ws, outside)
    }

    fn assert_link_refused(ws: &Path, outside: &Path) {
        let mut sb = Sandbox::new(ws, false).expect("sandbox");
        assert_eq!(sb.read("link/secret.txt"), Err(OUTSIDE.to_owned()));
        assert_eq!(sb.write("link/new.png", b"n"), Err(OUTSIDE.to_owned()));
        assert_eq!(sb.check_write("link/new.png"), Err(OUTSIDE.to_owned()));
        assert!(!outside.join("new.png").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_leading_out_is_refused() {
        let (ws, outside) = linked_workspace("sandbox-link", |target, link| {
            std::os::unix::fs::symlink(target, link).expect("symlink");
        });
        assert_link_refused(&ws, &outside);
    }

    /// Windows: a directory symlink, or — without the symlink privilege —
    /// a directory junction, which needs none.
    #[cfg(windows)]
    #[test]
    fn symlink_leading_out_is_refused() {
        let (ws, outside) = linked_workspace("sandbox-link", |target, link| {
            if std::os::windows::fs::symlink_dir(target, link).is_err() {
                let status = std::process::Command::new("cmd")
                    .arg("/C")
                    .arg("mklink")
                    .arg("/J")
                    .arg(link)
                    .arg(target)
                    .stdout(std::process::Stdio::null())
                    .status()
                    .expect("run mklink");
                assert!(status.success(), "mklink /J failed");
            }
        });
        assert_link_refused(&ws, &outside);
    }
}
