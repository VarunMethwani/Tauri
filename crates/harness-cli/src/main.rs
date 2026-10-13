use harness_core::SessionStore;
use harness_tools::Workspace;
use std::{env, path::PathBuf, process::Command};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("doctor") => {
            if args.next().is_some() {
                return Err("doctor takes no arguments".into());
            }
            doctor()
        }
        Some("inspect") => {
            let path = args.next().ok_or("usage: harness-cli inspect <workspace-path>")?;
            if args.next().is_some() {
                return Err("inspect takes one workspace path".into());
            }
            inspect(&path)
        }
        Some("search") => {
            let path = args.next().ok_or("usage: harness-cli search <workspace-path> <literal-query>")?;
            let query = args.next().ok_or("usage: harness-cli search <workspace-path> <literal-query>")?;
            if args.next().is_some() {
                return Err("search takes one workspace path and one literal query".into());
            }
            let workspace = Workspace::open(path)?;
            print!("{}", workspace.search_text(&query)?);
            Ok(())
        }
        _ => {
            eprintln!("usage: harness-cli <doctor | inspect <workspace-path> | search <workspace-path> <literal-query>>");
            Err("unknown or missing command".into())
        }
    }
}

fn doctor() -> Result<(), Box<dyn std::error::Error>> {
    println!("Agent Harness foundation diagnostics");
    for (label, binary, version_arg) in [
        ("Git", "git", "--version"),
        ("ripgrep", "rg", "--version"),
        ("Rust", "rustc", "--version"),
        ("Node", "node", "--version"),
        ("npm", "npm", "--version"),
        ("Codex CLI", "codex", "--version"),
        ("Claude Code", "claude", "--version"),
        ("Ollama", "ollama", "--version"),
    ] {
        let status = Command::new(binary).arg(version_arg).output();
        match status {
            Ok(output) if output.status.success() => {
                println!("{label}: {}", String::from_utf8_lossy(&output.stdout).trim());
            }
            _ => println!("{label}: unavailable"),
        }
    }
    #[cfg(target_os = "linux")]
    {
        let webkit = Command::new("pkg-config")
            .args(["--modversion", "webkit2gtk-4.1"])
            .output();
        match webkit {
            Ok(output) if output.status.success() => println!("WebKitGTK 4.1: {}", String::from_utf8_lossy(&output.stdout).trim()),
            _ => println!("WebKitGTK 4.1: unavailable (required for Tauri desktop)"),
        }
    }
    let db_path = data_dir()?.join("sessions.sqlite3");
    std::fs::create_dir_all(db_path.parent().ok_or("database path has no parent")?)?;
    let store = SessionStore::open(&db_path)?;
    println!("Session database: {} ({} sessions)", db_path.display(), store.list_sessions()?.len());
    println!("Provider: offline demo only; account authentication is not installed");
    println!("MCP, PTY, sandbox: not installed in this milestone");
    Ok(())
}

fn inspect(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = Workspace::open(path)?;
    let entries = workspace.list_files()?;
    println!("Workspace: {}", workspace.root().display());
    println!("Indexed entries: {} (limited to 5000)", entries.len());
    for entry in entries.iter().take(40) {
        println!("{} {}", if entry.is_dir { "dir " } else { "file" }, entry.path);
    }
    if entries.len() > 40 {
        println!("… {} more entries", entries.len() - 40);
    }
    match workspace.git_status() {
        Ok(status) if status.is_empty() => println!("Git: clean working tree"),
        Ok(status) => println!("Git status:\n{status}"),
        Err(_) => println!("Git: unavailable or not a repository"),
    }
    Ok(())
}

fn data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    Ok(base.ok_or("cannot locate a user data directory")?.join("agent-harness"))
}
