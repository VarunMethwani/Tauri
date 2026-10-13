use harness_core::{start_offline_turn as run_offline_turn, AgentEvent, CancellationHandle, Message, PermissionMode, Session, SessionStore};
use harness_tools::{FileEntry, Workspace};
use serde::Serialize;
use std::{collections::HashMap, env, path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager, State};

struct AppState {
    workspace: Mutex<Option<Workspace>>,
    turns: Mutex<HashMap<String, CancellationHandle>>,
    sessions: SessionStore,
}

#[derive(Serialize)]
struct WorkspaceInfo {
    name: String,
    path: String,
}

#[derive(Clone, Serialize)]
struct EventEnvelope {
    turn_id: String,
    event: AgentEvent,
}

fn locked_workspace(state: &State<'_, AppState>) -> Result<Workspace, String> {
    state.workspace.lock().map_err(|_| "workspace lock failed".to_string())?
        .clone().ok_or_else(|| "Open a workspace first".to_string())
}

#[tauri::command]
fn open_workspace(path: String, state: State<'_, AppState>) -> Result<WorkspaceInfo, String> {
    let workspace = Workspace::open(path).map_err(|error| error.to_string())?;
    let info = WorkspaceInfo { name: workspace.name(), path: workspace.root().display().to_string() };
    *state.workspace.lock().map_err(|_| "workspace lock failed".to_string())? = Some(workspace);
    Ok(info)
}

#[tauri::command]
fn list_files(state: State<'_, AppState>) -> Result<Vec<FileEntry>, String> {
    locked_workspace(&state)?.list_files().map_err(|error| error.to_string())
}

#[tauri::command]
fn read_file(path: String, state: State<'_, AppState>) -> Result<String, String> {
    locked_workspace(&state)?.read_file(&path).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_file(path: String, content: String, mode: PermissionMode, approved: bool, expected_content: Option<String>, state: State<'_, AppState>) -> Result<(), String> {
    locked_workspace(&state)?.write_file_if_unchanged(&path, &content, expected_content.as_deref(), mode, approved).map_err(|error| error.to_string())
}

#[tauri::command]
fn git_diff(state: State<'_, AppState>) -> Result<String, String> {
    locked_workspace(&state)?.git_diff().map_err(|error| error.to_string())
}

#[tauri::command]
fn git_status(state: State<'_, AppState>) -> Result<String, String> {
    locked_workspace(&state)?.git_status().map_err(|error| error.to_string())
}

#[tauri::command]
fn search_text(query: String, state: State<'_, AppState>) -> Result<String, String> {
    locked_workspace(&state)?.search_text(&query).map_err(|error| error.to_string())
}

#[tauri::command]
fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>, String> {
    state.sessions.list_sessions().map_err(|error| error.to_string())
}

#[tauri::command]
fn create_session(title: String, mode: PermissionMode, state: State<'_, AppState>) -> Result<Session, String> {
    let workspace = locked_workspace(&state)?;
    state.sessions.create_session(&workspace.root().display().to_string(), &title, mode).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_messages(session_id: String, state: State<'_, AppState>) -> Result<Vec<Message>, String> {
    state.sessions.messages(&session_id).map_err(|error| error.to_string())
}

#[tauri::command]
async fn start_offline_turn(session_id: String, prompt: String, mode: PermissionMode, turn_id: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if prompt.trim().is_empty() { return Err("Prompt is empty".to_string()); }
    let workspace_path = locked_workspace(&state)?.root().display().to_string();
    if state.sessions.list_sessions().map_err(|error| error.to_string())?.iter().all(|session| session.id != session_id || session.workspace != workspace_path) {
        return Err("Session not found".to_string());
    }
    state.sessions.append_message(&session_id, "user", &prompt).map_err(|error| error.to_string())?;
    let (mut receiver, cancellation) = run_offline_turn(prompt, mode);
    state.turns.lock().map_err(|_| "turn lock failed".to_string())?.insert(turn_id.clone(), cancellation);
    let sessions = state.sessions.clone();
    tauri::async_runtime::spawn(async move {
        let mut text = String::new();
        while let Some(event) = receiver.recv().await {
            if let AgentEvent::TextDelta { text: delta } = &event { text.push_str(delta); }
            let done = matches!(event, AgentEvent::Finished | AgentEvent::Cancelled | AgentEvent::Error { .. });
            if done && !text.is_empty() { let _ = sessions.append_message(&session_id, "assistant", &text); }
            let _ = app.emit("agent-event", EventEnvelope { turn_id: turn_id.clone(), event });
            if done { break; }
        }
        let state = app.state::<AppState>();
        if let Ok(mut turns) = state.turns.lock() { turns.remove(&turn_id); }
    });
    Ok(())
}

#[tauri::command]
fn cancel_turn(turn_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(handle) = state.turns.lock().map_err(|_| "turn lock failed".to_string())?.get(&turn_id) { handle.cancel(); }
    Ok(())
}

fn data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    base.map(|path| path.join("agent-harness")).ok_or_else(|| "Cannot locate user data directory".to_string())
}

fn main() {
    let directory = data_dir().expect("user data directory");
    std::fs::create_dir_all(&directory).expect("create user data directory");
    let sessions = SessionStore::open(directory.join("sessions.sqlite3")).expect("open session database");
    tauri::Builder::default()
        .manage(AppState { workspace: Mutex::new(None), turns: Mutex::new(HashMap::new()), sessions })
        .invoke_handler(tauri::generate_handler![
            open_workspace, list_files, read_file, write_file, git_diff, git_status, search_text,
            list_sessions, create_session, get_messages, start_offline_turn, cancel_turn
        ])
        .run(tauri::generate_context!())
        .expect("run Harness desktop app");
}
