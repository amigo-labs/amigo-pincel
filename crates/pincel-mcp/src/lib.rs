//! Pincel's MCP server: the `pincel-agent` tools over JSON-RPC 2.0, with a
//! workspace sandbox for file access. The `pincel-mcp` binary speaks it
//! newline-delimited over stdio. See docs/specs/mcp-headless-pixel.md and
//! the Decision Log in docs/specs/pincel.md §15 (2026-10-29).

pub mod base64;
pub mod sandbox;
pub mod server;

use std::path::PathBuf;

/// `--help` text.
pub const USAGE: &str = "\
pincel-mcp — Pincel MCP server (stdio)

Usage: pincel-mcp [--workspace <dir>] [--read-only]

Options:
  --workspace <dir>  Directory every path argument resolves inside (default: current directory)
  --read-only        Refuse doc.save / export.* (in-memory tools keep working)
  -h, --help         Print this help
  -V, --version      Print the version

Register with Claude Code:
  claude mcp add pincel -- pincel-mcp --workspace .

Exit codes: 0 stdin closed, 2 usage error, 3 I/O error (workspace or stdio).";

/// Parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cli {
    /// Serve over stdio.
    Serve {
        /// Workspace directory; `None` = current directory.
        workspace: Option<PathBuf>,
        /// Refuse writes.
        read_only: bool,
    },
    /// Print help.
    Help,
    /// Print the version.
    Version,
}

/// Parse the arguments after the program name.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Cli, String> {
    let mut workspace = None;
    let mut read_only = false;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Cli::Help),
            "-V" | "--version" => return Ok(Cli::Version),
            "--read-only" => read_only = true,
            "--workspace" => {
                let dir = it
                    .next()
                    .ok_or_else(|| "--workspace needs a directory".to_owned())?;
                workspace = Some(PathBuf::from(dir));
            }
            other => match other.strip_prefix("--workspace=") {
                Some(dir) => workspace = Some(PathBuf::from(dir)),
                None => return Err(format!("unknown argument `{other}`")),
            },
        }
    }
    Ok(Cli::Serve {
        workspace,
        read_only,
    })
}

/// A fresh, empty directory under the system temp dir for one test.
#[cfg(test)]
pub(crate) fn test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pincel-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn parse_args_reads_workspace_and_read_only() {
        assert_eq!(
            parse(&[]),
            Ok(Cli::Serve {
                workspace: None,
                read_only: false
            })
        );
        assert_eq!(
            parse(&["--workspace", "art", "--read-only"]),
            Ok(Cli::Serve {
                workspace: Some(PathBuf::from("art")),
                read_only: true
            })
        );
        assert_eq!(
            parse(&["--workspace=x"]),
            Ok(Cli::Serve {
                workspace: Some(PathBuf::from("x")),
                read_only: false
            })
        );
        assert_eq!(parse(&["--help"]), Ok(Cli::Help));
        assert!(parse(&["--workspace"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }
}
