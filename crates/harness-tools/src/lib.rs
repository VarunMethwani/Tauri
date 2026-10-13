//! Workspace-scoped operations. The caller supplies a permission mode for each
//! mutation; the Rust side enforces it even if the UI is bypassed.

use harness_core::{evaluate_permission, Operation, PermissionDecision, PermissionMode};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};
use thiserror::Error;

const MAX_READ_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 5000;
const MAX_DEPTH: usize = 12;
const MAX_COMMAND_OUTPUT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("workspace path must be a directory")]
    InvalidWorkspace,
    #[error("file path must be relative to the workspace")]
    InvalidPath,
    #[error("path resolves outside the workspace")]
    OutsideWorkspace,
    #[error("file is too large for the text editor (2 MiB limit)")]
    TooLarge,
    #[error("file is not valid UTF-8 text")]
    NotText,
    #[error("file changed on disk since it was opened; reload before saving")]
    Conflict,
    #[error("command output exceeds the 2 MiB display limit")]
    OutputTooLarge,
    #[error("this permission mode denies workspace file edits")]
    Denied,
    #[error("this file edit requires explicit approval")]
    ApprovalRequired,
    #[error("Git is not available or this folder is not a Git repository: {0}")]
    Git(String),
    #[error("Search failed: {0}")]
    Search(String),
    #[error("search query must not be empty")]
    EmptyQuery,
}

pub type Result<T> = std::result::Result<T, ToolError>;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(path)?;
        if !root.is_dir() {
            return Err(ToolError::InvalidWorkspace);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> String {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.root.display().to_string())
    }

    pub fn list_files(&self) -> Result<Vec<FileEntry>> {
        let mut entries = Vec::new();
        let mut pending = vec![(self.root.clone(), 0usize)];
        while let Some((directory, depth)) = pending.pop() {
            let mut children = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                if entries.len() >= MAX_ENTRIES {
                    return Ok(entries);
                }
                let path = child.path();
                let name = child.file_name().to_string_lossy().into_owned();
                if matches!(name.as_str(), ".git" | "node_modules" | "target" | "dist" | ".repo-agent") {
                    continue;
                }
                let metadata = fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                let is_dir = metadata.is_dir();
                if !(is_dir || metadata.is_file()) {
                    continue;
                }
                let relative = path.strip_prefix(&self.root).map_err(|_| ToolError::OutsideWorkspace)?;
                entries.push(FileEntry {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    name,
                    is_dir,
                });
                if is_dir && depth < MAX_DEPTH {
                    pending.push((path, depth + 1));
                }
            }
        }
        Ok(entries)
    }

    pub fn read_file(&self, path: &str) -> Result<String> {
        let target = self.resolve_existing(path)?;
        let metadata = fs::metadata(&target)?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidPath);
        }
        if metadata.len() > MAX_READ_BYTES {
            return Err(ToolError::TooLarge);
        }
        let mut file = fs::File::open(target)?;
        let mut bytes = Vec::new();
        file.take(MAX_READ_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_READ_BYTES {
            return Err(ToolError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| ToolError::NotText)
    }

    pub fn write_file(
        &self,
        path: &str,
        content: &str,
        mode: PermissionMode,
        approved: bool,
    ) -> Result<()> {
        self.write_file_if_unchanged(path, content, None, mode, approved)
    }

    pub fn write_file_if_unchanged(
        &self,
        path: &str,
        content: &str,
        expected_content: Option<&str>,
        mode: PermissionMode,
        approved: bool,
    ) -> Result<()> {
        match evaluate_permission(mode, Operation::WorkspaceWrite) {
            PermissionDecision::Deny => return Err(ToolError::Denied),
            PermissionDecision::Ask if !approved => return Err(ToolError::ApprovalRequired),
            _ => {}
        }
        if content.len() as u64 > MAX_READ_BYTES {
            return Err(ToolError::TooLarge);
        }
        let target = self.resolve_existing(path)?;
        let metadata = fs::metadata(&target)?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidPath);
        }
        if metadata.len() > MAX_READ_BYTES {
            return Err(ToolError::TooLarge);
        }
        if let Some(expected) = expected_content {
            if self.read_file(path)? != expected {
                return Err(ToolError::Conflict);
            }
        }
        let mut file = fs::OpenOptions::new().write(true).truncate(true).open(target)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    pub fn git_diff(&self) -> Result<String> {
        self.run_git(&["diff", "--no-ext-diff", "--no-color", "--"])
    }

    pub fn git_status(&self) -> Result<String> {
        self.run_git(&["status", "--short", "--untracked-files=normal", "--"])
    }

    /// Literal text search with ripgrep's ignore and symlink defaults. Output
    /// is bounded so a broad query cannot flood the UI or agent context.
    pub fn search_text(&self, query: &str) -> Result<String> {
        if query.trim().is_empty() {
            return Err(ToolError::EmptyQuery);
        }
        let mut child = Command::new("rg")
            .current_dir(&self.root)
            .args(["--fixed-strings", "--line-number", "--no-heading", "--color", "never", "--max-count", "20", "--"])
            .arg(query)
            .arg(".")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let output = read_bounded_output(&mut child)?;
        match child.wait()?.code() {
            Some(0) => String::from_utf8(output).map_err(|_| ToolError::NotText),
            Some(1) => Ok(String::new()),
            _ => Err(ToolError::Search("ripgrep could not complete the search".into())),
        }
    }

    fn run_git(&self, arguments: &[&str]) -> Result<String> {
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(arguments)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let output = read_bounded_output(&mut child)?;
        if !child.wait()?.success() {
            return Err(ToolError::Git("Git command failed; confirm this folder is a Git repository".into()));
        }
        String::from_utf8(output).map_err(|_| ToolError::NotText)
    }

    fn resolve_existing(&self, path: &str) -> Result<PathBuf> {
        let relative = Path::new(path);
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ToolError::InvalidPath);
        }
        let target = fs::canonicalize(self.root.join(relative))?;
        if !target.starts_with(&self.root) {
            return Err(ToolError::OutsideWorkspace);
        }
        Ok(target)
    }
}

fn read_bounded_output(child: &mut std::process::Child) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let read_result = child.stdout.take().ok_or_else(|| ToolError::Git("Could not capture command output".into()))?
        .take(MAX_COMMAND_OUTPUT_BYTES + 1).read_to_end(&mut output);
    if let Err(error) = read_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ToolError::Io(error));
    }
    if output.len() as u64 > MAX_COMMAND_OUTPUT_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(ToolError::OutputTooLarge);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("harness-tools-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "before").unwrap();
        root
    }

    #[test]
    fn plan_and_ask_cannot_write_without_authorization() {
        let root = fixture();
        let workspace = Workspace::open(&root).unwrap();
        assert!(matches!(workspace.write_file("note.txt", "after", PermissionMode::Plan, true), Err(ToolError::Denied)));
        assert!(matches!(workspace.write_file("note.txt", "after", PermissionMode::Ask, false), Err(ToolError::ApprovalRequired)));
        assert_eq!(workspace.read_file("note.txt").unwrap(), "before");
        workspace.write_file("note.txt", "after", PermissionMode::Ask, true).unwrap();
        assert_eq!(workspace.read_file("note.txt").unwrap(), "after");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn traversal_is_rejected() {
        let root = fixture();
        let workspace = Workspace::open(&root).unwrap();
        assert!(matches!(workspace.read_file("../outside"), Err(ToolError::InvalidPath)));
        assert!(matches!(workspace.read_file("/etc/passwd"), Err(ToolError::InvalidPath)));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_outside_workspace_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = fixture();
        symlink("/etc/passwd", root.join("outside.txt")).unwrap();
        let workspace = Workspace::open(&root).unwrap();
        assert!(matches!(workspace.read_file("outside.txt"), Err(ToolError::OutsideWorkspace)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_editor_content_cannot_overwrite_a_newer_file() {
        let root = fixture();
        let workspace = Workspace::open(&root).unwrap();
        fs::write(root.join("note.txt"), "newer disk content").unwrap();
        assert!(matches!(
            workspace.write_file_if_unchanged("note.txt", "my old edit", Some("before"), PermissionMode::AutoEdit, false),
            Err(ToolError::Conflict)
        ));
        assert_eq!(workspace.read_file("note.txt").unwrap(), "newer disk content");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn git_status_includes_untracked_files() {
        let root = fixture();
        let initialized = Command::new("git").arg("init").arg("-q").arg(&root).status().unwrap();
        assert!(initialized.success());
        let workspace = Workspace::open(&root).unwrap();
        assert!(workspace.git_status().unwrap().contains("?? note.txt"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn literal_search_returns_workspace_match() {
        let root = fixture();
        let workspace = Workspace::open(&root).unwrap();
        assert!(workspace.search_text("before").unwrap().contains("note.txt:1:before"));
        assert!(workspace.search_text("missing phrase").unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
