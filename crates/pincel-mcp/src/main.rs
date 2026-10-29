//! `pincel-mcp`: serve the Pincel agent tools over MCP on stdin / stdout,
//! one JSON-RPC message per line.

use std::io::{BufRead, Write};
use std::process::ExitCode;

use pincel_mcp::sandbox::Sandbox;
use pincel_mcp::server::Server;
use pincel_mcp::{Cli, USAGE, parse_args};

fn main() -> ExitCode {
    let (workspace, read_only) = match parse_args(std::env::args().skip(1)) {
        Ok(Cli::Serve {
            workspace,
            read_only,
        }) => (workspace, read_only),
        Ok(Cli::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Cli::Version) => {
            println!("pincel-mcp {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("pincel-mcp: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let dir = match workspace {
        Some(dir) => dir,
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("pincel-mcp: cannot read the current directory: {e}");
                return ExitCode::from(3);
            }
        },
    };
    let sandbox = match Sandbox::new(&dir, read_only) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pincel-mcp: workspace `{}`: {e}", dir.display());
            return ExitCode::from(3);
        }
    };
    let mut server = Server::new(sandbox);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("pincel-mcp: stdin: {e}");
                return ExitCode::from(3);
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_text(&line)
            && writeln!(stdout, "{response}")
                .and_then(|()| stdout.flush())
                .is_err()
        {
            return ExitCode::from(3);
        }
    }
    ExitCode::SUCCESS
}
