import {
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { bridge, isDesktop } from "./bridge";
import type { AgentEvent, Message, PermissionMode, Session } from "./types";

type Activity = "files" | "search" | "git" | "sessions" | "providers";
type FileEntry = { path: string; name: string; is_dir: boolean };
type OutputEntry = {
  id: number;
  kind: "info" | "tool" | "success" | "error";
  text: string;
};

type GitStatusEntry = {
  code: string;
  path: string;
};

const runtimeLabel = isDesktop ? "Native runtime" : "Browser preview";
const providerLabel = isDesktop ? "Offline provider" : "Local demo";

const MODE_LABELS: Record<Exclude<PermissionMode, "autonomous">, string> = {
  plan: "Plan",
  ask: "Ask",
  auto_edit: "Auto Edit",
};

function Icon({ name, size = 16 }: { name: string; size?: number }) {
  const paths: Record<string, ReactNode> = {
    files: <><path d="M4 3.5h5l2 2H20v14H4z" /><path d="M4 8h16" /></>,
    git: <><circle cx="6" cy="5" r="2" /><circle cx="18" cy="6" r="2" /><circle cx="6" cy="19" r="2" /><path d="M6 7v10M8 6h4a6 6 0 0 1 6 6v-4" /></>,
    sessions: <><path d="M4 5h16v12H8l-4 3z" /><path d="M8 9h8M8 13h5" /></>,
    providers: <><path d="M9 4.5v5M15 4.5v5M7 9.5h10v2a5 5 0 0 1-5 5v3" /><path d="M9 4.5h6" /></>,
    search: <><circle cx="10.8" cy="10.8" r="6.2" /><path d="m16 16 4.5 4.5" /></>,
    chevron: <path d="m9 18 6-6-6-6" />,
    file: <><path d="M6 3h8l4 4v14H6z" /><path d="M14 3v5h5" /></>,
    folder: <path d="M3 6h7l2 2h9v11H3z" />,
    refresh: <><path d="M20 7v5h-5" /><path d="M19 12a7 7 0 1 0-1.5 4.4" /></>,
    save: <><path d="M5 3h12l3 3v15H4V3z" /><path d="M8 3v6h8V3M8 21v-7h8v7" /></>,
    plus: <path d="M12 5v14M5 12h14" />,
    send: <path d="m4 4 17 8-17 8 3-8zM7 12h14" />,
    stop: <rect x="6" y="6" width="12" height="12" rx="1" />,
    terminal: <><path d="m5 7 4 4-4 4M11 16h7" /><rect x="3" y="3" width="18" height="18" rx="2" /></>,
    x: <path d="m6 6 12 12M18 6 6 18" />,
    check: <path d="m5 12 4 4L19 6" />,
    alert: <><path d="M12 9v4M12 17h.01" /><path d="M10.3 3.8 2.6 18a2 2 0 0 0 1.8 3h15.2a2 2 0 0 0 1.8-3L13.7 3.8a2 2 0 0 0-3.4 0Z" /></>,
    spark: <><path d="m12 3 .8 4.2L17 8l-4.2.8L12 13l-.8-4.2L7 8l4.2-.8z" /><path d="m18 14 .5 2.5L21 17l-2.5.5L18 20l-.5-2.5L15 17l2.5-.5z" /></>,
  };
  return <svg aria-hidden="true" className="icon" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">{paths[name]}</svg>;
}

function fileKind(path: string) {
  const ext = path.split(".").pop()?.toLowerCase();
  if (["ts", "tsx", "js", "jsx"].includes(ext ?? "")) return "ts";
  if (["css", "scss"].includes(ext ?? "")) return "css";
  if (["json", "toml", "yaml", "yml"].includes(ext ?? "")) return "config";
  if (["md", "mdx", "txt"].includes(ext ?? "")) return "doc";
  return "plain";
}

function modeForSession(mode: PermissionMode): Exclude<PermissionMode, "autonomous"> {
  return mode === "autonomous" ? "auto_edit" : mode;
}

function parseGitStatus(status: string): GitStatusEntry[] {
  return status
    .split("\n")
    .map((line) => line.trimEnd())
    .filter(Boolean)
    .map((line) => ({ code: line.slice(0, 2).replace(/ /g, "·") || "?", path: line.slice(3).trim() || line.trim() }));
}

function isNotGitError(message: string) {
  return /not a git repository|git is not available|git command failed/i.test(message);
}

export default function App() {
  const [activity, setActivity] = useState<Activity>("files");
  const [mobileAgentOpen, setMobileAgentOpen] = useState(false);
  const [workspacePath, setWorkspacePath] = useState(isDesktop ? "" : "/demo/agent-harness");
  const [workspace, setWorkspace] = useState<{ name: string; path: string } | null>(null);
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [filesState, setFilesState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const [filesError, setFilesError] = useState("");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState("");
  const [savedContent, setSavedContent] = useState("");
  const [fileLoading, setFileLoading] = useState(false);
  const [fileError, setFileError] = useState("");
  const [mode, setMode] = useState<Exclude<PermissionMode, "autonomous">>("ask");
  const [saveApproval, setSaveApproval] = useState(false);
  const [saving, setSaving] = useState(false);
  const [diff, setDiff] = useState("");
  const [diffState, setDiffState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const [diffError, setDiffError] = useState("");
  const [gitStatus, setGitStatus] = useState("");
  const [gitStatusState, setGitStatusState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const [gitStatusError, setGitStatusError] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState("");
  const [searchState, setSearchState] = useState<"idle" | "loading" | "ready" | "error">("idle");
  const [searchError, setSearchError] = useState("");
  const [sessions, setSessions] = useState<Session[]>([]);
  const [sessionState, setSessionState] = useState<"loading" | "ready" | "error">("loading");
  const [sessionError, setSessionError] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [messagesLoading, setMessagesLoading] = useState(false);
  const [prompt, setPrompt] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [cancelTurn, setCancelTurn] = useState<(() => void) | null>(null);
  const [turnError, setTurnError] = useState("");
  const [permissionOperation, setPermissionOperation] = useState<string | null>(null);
  const [output, setOutput] = useState<OutputEntry[]>([]);
  const [outputOpen, setOutputOpen] = useState(true);
  const outputId = useRef(0);
  const streamId = useRef(0);
  const skipSessionLoad = useRef<string | null>(null);
  const fileRequestId = useRef(0);
  const fileListRequestId = useRef(0);
  const sessionRequestId = useRef(0);
  const gitRequestId = useRef(0);
  const searchRequestId = useRef(0);
  const messageEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const saveCancelRef = useRef<HTMLButtonElement>(null);
  const saveFocusRef = useRef<HTMLElement | null>(null);

  const dirty = selectedFile !== null && fileContent !== savedContent;
  const activeSession = sessions.find((item) => item.id === activeSessionId) ?? null;

  const addOutput = useCallback((kind: OutputEntry["kind"], text: string) => {
    setOutput((current) => [...current.slice(-99), { id: outputId.current++, kind, text }]);
  }, []);

  const loadFiles = useCallback(async () => {
    const request = ++fileListRequestId.current;
    setFilesState("loading");
    setFilesError("");
    try {
      const next = await bridge.listFiles();
      if (request !== fileListRequestId.current) return;
      setFiles(next);
      setFilesState("ready");
      addOutput("success", `Workspace index ready: ${next.length} ${next.length === 1 ? "entry" : "entries"}.`);
    } catch (error) {
      if (request !== fileListRequestId.current) return;
      const message = error instanceof Error ? error.message : String(error);
      setFilesError(message);
      setFilesState("error");
      addOutput("error", `Could not list workspace: ${message}`);
    }
  }, [addOutput]);

  const loadSessions = useCallback(async (workspaceForSessions?: string) => {
    const request = ++sessionRequestId.current;
    setSessionState("loading");
    setSessionError("");
    try {
      const all = await bridge.listSessions();
      if (request !== sessionRequestId.current) return;
      const path = workspaceForSessions ?? workspace?.path;
      const next = path ? all.filter((item) => item.workspace === path) : [];
      setSessions(next);
      setSessionState("ready");
      setActiveSessionId((current) => next.some((item) => item.id === current) ? current : next[0]?.id ?? null);
    } catch (error) {
      if (request !== sessionRequestId.current) return;
      setSessionError(error instanceof Error ? error.message : String(error));
      setSessionState("error");
    }
  }, [workspace?.path]);

  useEffect(() => { void loadSessions(); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!activeSessionId) {
      setMessages([]);
      return;
    }
    if (skipSessionLoad.current === activeSessionId) {
      skipSessionLoad.current = null;
      setMessagesLoading(false);
      return;
    }
    const request = ++streamId.current;
    setMessagesLoading(true);
    setTurnError("");
    bridge.getMessages(activeSessionId)
      .then((next) => { if (request === streamId.current) setMessages(next); })
      .catch((error) => { if (request === streamId.current) setTurnError(error instanceof Error ? error.message : String(error)); })
      .finally(() => { if (request === streamId.current) setMessagesLoading(false); });
  }, [activeSessionId]);

  useEffect(() => {
    if (!saveApproval) return;
    requestAnimationFrame(() => saveCancelRef.current?.focus());
  }, [saveApproval]);

  useEffect(() => {
    if (!saveApproval) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) {
        setSaveApproval(false);
        requestAnimationFrame(() => (saveFocusRef.current ?? editorRef.current)?.focus());
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [saveApproval, saving]);

  useEffect(() => { messageEndRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" }); }, [messages, streaming]);

  async function openWorkspace(event?: FormEvent) {
    event?.preventDefault();
    const nextPath = workspacePath.trim();
    if (!nextPath) return;
    if (streaming) {
      setFilesError("Cancel the active agent turn before switching workspaces.");
      addOutput("info", "Workspace switch blocked while an agent turn is active.");
      return;
    }
    if (dirty && !window.confirm("You have unsaved editor changes. Switch workspaces and discard them?")) return;
    setFilesState("loading");
    setFilesError("");
    try {
      const nextWorkspace = await bridge.openWorkspace(nextPath);
      setWorkspace(nextWorkspace);
      setWorkspacePath(nextWorkspace.path);
      setActiveSessionId(null);
      setMessages([]);
      streamId.current += 1;
      fileRequestId.current += 1;
      fileListRequestId.current += 1;
      sessionRequestId.current += 1;
      gitRequestId.current += 1;
      searchRequestId.current += 1;
      setSelectedFile(null);
      setFileContent("");
      setSavedContent("");
      setGitStatus("");
      setGitStatusState("idle");
      setGitStatusError("");
      setDiff("");
      setDiffState("idle");
      setDiffError("");
      setSearchQuery("");
      setSearchResults("");
      setSearchState("idle");
      setSearchError("");
      addOutput("success", `Opened ${nextWorkspace.path}`);
      await loadFiles();
      await loadSessions(nextWorkspace.path);
      if (activity === "git") await loadGit();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setFilesState("error");
      setFilesError(message);
      addOutput("error", `Could not open workspace: ${message}`);
    }
  }

  async function selectFile(path: string) {
    if (path === selectedFile) return;
    if (dirty && !window.confirm("Discard your unsaved editor changes?")) return;
    const request = ++fileRequestId.current;
    setSelectedFile(path);
    setFileLoading(true);
    setFileError("");
    try {
      const content = await bridge.readFile(path);
      if (request !== fileRequestId.current) return;
      setFileContent(content);
      setSavedContent(content);
    } catch (error) {
      if (request !== fileRequestId.current) return;
      const message = error instanceof Error ? error.message : String(error);
      setFileError(message);
      setFileContent("");
      setSavedContent("");
      addOutput("error", `Could not read ${path}: ${message}`);
    } finally {
      if (request === fileRequestId.current) setFileLoading(false);
    }
  }

  const saveFile = useCallback(async (approved = false) => {
    if (!selectedFile || !dirty || saving || mode === "plan") return;
    if (mode === "ask" && !approved) {
      saveFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : editorRef.current;
      setSaveApproval(true);
      return;
    }
    setSaving(true);
    try {
      await bridge.writeFile(selectedFile, fileContent, mode, approved, savedContent);
      setSavedContent(fileContent);
      setSaveApproval(false);
      addOutput("success", `Saved ${selectedFile}`);
      if (activity === "git") void loadGit();
      requestAnimationFrame(() => (saveFocusRef.current ?? editorRef.current)?.focus());
    } catch (error) {
      addOutput("error", `Save failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setSaving(false);
    }
  }, [activity, addOutput, dirty, fileContent, mode, savedContent, saving, selectedFile]);

  useEffect(() => {
    const saveOnShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        void saveFile();
      }
    };
    window.addEventListener("keydown", saveOnShortcut);
    return () => window.removeEventListener("keydown", saveOnShortcut);
  }, [saveFile]);

  async function loadGit() {
    const request = ++gitRequestId.current;
    setGitStatusState("loading");
    setGitStatusError("");
    setDiffState("loading");
    setDiffError("");
    const [statusResult, diffResult] = await Promise.allSettled([bridge.gitStatus(), bridge.gitDiff()]);
    if (request !== gitRequestId.current) return;

    if (statusResult.status === "fulfilled") {
      setGitStatus(statusResult.value);
      setGitStatusState("ready");
    } else {
      const message = statusResult.reason instanceof Error ? statusResult.reason.message : String(statusResult.reason);
      setGitStatusError(message);
      setGitStatusState("error");
    }
    if (diffResult.status === "fulfilled") {
      setDiff(diffResult.value);
      setDiffState("ready");
    } else {
      const message = diffResult.reason instanceof Error ? diffResult.reason.message : String(diffResult.reason);
      setDiffError(message);
      setDiffState("error");
    }
  }

  useEffect(() => { if (activity === "git") void loadGit(); }, [activity]); // eslint-disable-line react-hooks/exhaustive-deps

  async function runSearch(event?: FormEvent) {
    event?.preventDefault();
    const query = searchQuery.trim();
    if (!query) {
      setSearchResults("");
      setSearchState("error");
      setSearchError("Enter a search term.");
      return;
    }
    setSearchState("loading");
    setSearchError("");
    const request = ++searchRequestId.current;
    try {
      const results = await bridge.searchText(query);
      if (request !== searchRequestId.current) return;
      setSearchResults(results);
      setSearchState("ready");
    } catch (error) {
      if (request !== searchRequestId.current) return;
      setSearchError(error instanceof Error ? error.message : String(error));
      setSearchState("error");
    }
  }

  async function createSession() {
    if (!workspace) {
      setSessionError("Open a workspace first.");
      return;
    }
    try {
      const session = await bridge.createSession("New session", mode);
      setSessions((current) => [session, ...current]);
      skipSessionLoad.current = session.id;
      setActiveSessionId(session.id);
      setMessages([]);
      setPermissionOperation(null);
      setActivity("sessions");
      requestAnimationFrame(() => textareaRef.current?.focus());
    } catch (error) {
      setSessionError(error instanceof Error ? error.message : String(error));
    }
  }

  function selectSession(session: Session) {
    if (streaming) return;
    setActiveSessionId(session.id);
    setMode(modeForSession(session.mode));
    setPermissionOperation(null);
  }

  async function submitPrompt(event?: FormEvent) {
    event?.preventDefault();
    const text = prompt.trim();
    if (!text || streaming) return;
    if (!workspace) {
      setTurnError("Open a workspace first.");
      return;
    }
    let sessionId = activeSessionId;
    if (!sessionId) {
      try {
        const session = await bridge.createSession(text.slice(0, 42), mode);
        setSessions((current) => [session, ...current]);
        skipSessionLoad.current = session.id;
        setActiveSessionId(session.id);
        sessionId = session.id;
      } catch (error) {
        setTurnError(error instanceof Error ? error.message : String(error));
        return;
      }
    }
    setPrompt("");
    setTurnError("");
    setPermissionOperation(null);
    setStreaming(true);
    setMessages((current) => [...current, { role: "user", content: text }, { role: "assistant", content: "" }]);
    addOutput("info", "Turn started.");
    const currentStream = ++streamId.current;
    let settled = false;
    try {
      const cancel = await bridge.sendPrompt(sessionId, text, mode, (event: AgentEvent) => {
        if (currentStream !== streamId.current) return;
        if (event.type === "text_delta") {
          setMessages((current) => {
            const next = [...current];
            const last = next.at(-1);
            if (last?.role === "assistant") next[next.length - 1] = { ...last, content: last.content + event.text };
            return next;
          });
        } else if (event.type === "tool_started") addOutput("tool", `Started ${event.name}`);
        else if (event.type === "tool_finished") addOutput("success", `${event.name}: ${event.output}`);
        else if (event.type === "permission_required") {
          setPermissionOperation(event.operation);
          addOutput("info", `Permission required: ${event.operation}`);
        }
        else if (event.type === "finished") {
          settled = true;
          setStreaming(false);
          setCancelTurn(null);
          addOutput("success", "Turn finished.");
        } else if (event.type === "cancelled") {
          settled = true;
          setStreaming(false);
          setCancelTurn(null);
          addOutput("info", "Turn cancelled.");
        } else if (event.type === "error") {
          settled = true;
          setStreaming(false);
          setCancelTurn(null);
          setTurnError(event.message);
          addOutput("error", event.message);
        }
      });
      if (!settled && currentStream === streamId.current) setCancelTurn(() => cancel);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStreaming(false);
      setTurnError(message);
      addOutput("error", message);
    }
  }

  function cancel() {
    cancelTurn?.();
    streamId.current += 1;
    setCancelTurn(null);
    setStreaming(false);
    setPermissionOperation(null);
    addOutput("info", "Cancellation requested.");
  }

  function handlePromptKeyDown(event: ReactKeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void submitPrompt();
    }
  }

  const groupedFiles = useMemo(() => [...files].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.path.localeCompare(b.path);
  }), [files]);
  const gitEntries = useMemo(() => parseGitStatus(gitStatus), [gitStatus]);

  return (
    <div className={`app-shell ${isDesktop ? "" : "with-demo-banner"}`}>
      <header className="titlebar">
        <div className="wordmark"><span className="wordmark-mark"><Icon name="spark" size={14} /></span><span>Harness</span></div>
        <div className="titlebar-center" title={workspace?.path}>{workspace ? workspace.name : "Universal agent harness"}</div>
        <div className="titlebar-end"><div className="runtime"><span className="status-dot" /><span>{runtimeLabel}</span></div><button className="mobile-agent-toggle" onClick={() => setMobileAgentOpen((value) => !value)} aria-expanded={mobileAgentOpen} aria-controls="agent-panel">{mobileAgentOpen ? "Close agent" : "Agent"}</button></div>
      </header>

      {!isDesktop && <div className="demo-banner"><Icon name="alert" size={14} /><span>Browser preview uses a local demo workspace. File edits and sessions last for this tab.</span></div>}

      <div className="workbench">
        <nav className="activity-bar" aria-label="Workbench">
          <div className="activity-items">
            {(["files", "search", "git", "sessions", "providers"] as Activity[]).map((item) => (
              <button key={item} className={`activity-button ${activity === item ? "active" : ""}`} onClick={() => { setActivity(item); setMobileAgentOpen(false); }} aria-label={item[0].toUpperCase() + item.slice(1)} aria-pressed={activity === item} title={item[0].toUpperCase() + item.slice(1)}>
                <Icon name={item} size={19} />
              </button>
            ))}
          </div>
          <button className="activity-button muted" aria-label="Terminal unavailable" title="Terminal is not available in this foundation build" disabled><Icon name="terminal" size={19} /></button>
        </nav>

        <aside className="explorer" aria-label={`${activity} panel`}>
          <div className="panel-heading">
            <span>{activity}</span>
            {activity === "files" && <button className="icon-button" onClick={() => void loadFiles()} disabled={!workspace || filesState === "loading"} aria-label="Refresh files"><Icon name="refresh" size={14} /></button>}
            {activity === "git" && <button className="icon-button" onClick={() => void loadGit()} disabled={gitStatusState === "loading" || diffState === "loading"} aria-label="Refresh Git status and diff"><Icon name="refresh" size={14} /></button>}
            {activity === "sessions" && <button className="icon-button" onClick={() => void createSession()} disabled={streaming} aria-label="New session"><Icon name="plus" size={14} /></button>}
          </div>

          {activity === "search" && <>
            <form className="search-form" onSubmit={runSearch}>
              <label htmlFor="workspace-search">Search workspace</label>
              <div className="search-input-row"><input id="workspace-search" value={searchQuery} onChange={(event) => setSearchQuery(event.target.value)} placeholder="Search text…" autoComplete="off" disabled={!workspace || searchState === "loading"} /><button type="submit" disabled={!workspace || !searchQuery.trim() || searchState === "loading"}>Find</button></div>
            </form>
            <div className="panel-scroll search-panel" aria-live="polite" aria-busy={searchState === "loading"}>
              {searchState === "idle" && <EmptyState title={workspace ? "Search this workspace" : "Open a workspace"} detail={workspace ? "Enter a term to find matching lines in workspace files." : "Search is available after a workspace is open."} />}
              {searchState === "loading" && <LoadingRows />}
              {searchState === "error" && <InlineError text={searchError} retry={() => void runSearch()} />}
              {searchState === "ready" && !searchResults.trim() && <EmptyState title="No matches" detail={`No files contain “${searchQuery.trim()}”.`} />}
              {searchState === "ready" && searchResults.trim() && <pre className="search-results">{searchResults.split("\n").filter(Boolean).map((line, index) => <span key={`${line}-${index}`}>{line}{"\n"}</span>)}</pre>}
            </div>
          </>}

          {activity === "files" && <>
            <form className="workspace-form" onSubmit={openWorkspace}>
              <label htmlFor="workspace-path">Workspace path</label>
              <div className="inline-input"><input id="workspace-path" value={workspacePath} onChange={(event) => setWorkspacePath(event.target.value)} placeholder="/path/to/project" autoComplete="off" /><button type="submit" disabled={!workspacePath.trim() || filesState === "loading"}>Open</button></div>
            </form>
            <div className="tree-header"><span>{workspace?.name ?? "No workspace"}</span>{workspace && <span className="subtle">{groupedFiles.length}</span>}</div>
            <div className="panel-scroll file-tree">
              {filesState === "idle" && <EmptyState title="Open a workspace" detail="Enter a path above to browse its files." />}
              {filesState === "loading" && <LoadingRows />}
              {filesState === "error" && <InlineError text={filesError} retry={() => void openWorkspace()} />}
              {filesState === "ready" && groupedFiles.length === 0 && <EmptyState title="Workspace is empty" detail="No files are available to display." />}
              {filesState === "ready" && groupedFiles.map((file) => {
                const depth = Math.max(0, file.path.split("/").length - 1);
                return <button key={file.path} className={`tree-row ${selectedFile === file.path ? "selected" : ""}`} style={{ paddingLeft: 10 + Math.min(depth, 4) * 12 }} onClick={() => file.is_dir ? undefined : void selectFile(file.path)} disabled={file.is_dir} title={file.path}>
                  <span className={`file-icon ${file.is_dir ? "dir" : fileKind(file.path)}`}><Icon name={file.is_dir ? "folder" : "file"} size={14} /></span><span className="truncate">{file.name}</span>
                </button>;
              })}
            </div>
          </>}

          {activity === "git" && <div className="panel-scroll diff-panel" aria-busy={gitStatusState === "loading" || diffState === "loading"}>
            {(gitStatusState === "loading" || diffState === "loading") && <LoadingRows />}
            {gitStatusState === "error" && isNotGitError(gitStatusError) && <EmptyState title="Not a Git repository" detail="Open a folder with a .git directory to inspect status and diffs." action={<button className="secondary-button" onClick={() => void loadGit()}>Try again</button>} />}
            {gitStatusState === "error" && !isNotGitError(gitStatusError) && <InlineError text={gitStatusError} retry={() => void loadGit()} />}
            {gitStatusState === "ready" && <section className="git-status-block" aria-label="Git status">
              <div className="git-section-heading"><span>Working tree</span><span className={`git-count ${gitEntries.length ? "changed" : "clean"}`}>{gitEntries.length ? `${gitEntries.length} ${gitEntries.length === 1 ? "change" : "changes"}` : "clean"}</span></div>
              {gitEntries.length === 0 ? <p className="git-clean"><span className="git-clean-dot" />No local changes</p> : <ul className="git-status-list">{gitEntries.map((entry, index) => <li key={`${entry.path}-${index}`}><code className="git-status-code">{entry.code}</code><span className="truncate" title={entry.path}>{entry.path}</span></li>)}</ul>}
            </section>}
            {diffState === "error" && gitStatusState !== "error" && <InlineError text={diffError} retry={() => void loadGit()} />}
            {diffState === "ready" && <section className="git-diff-block" aria-label="Git diff">
              <div className="git-section-heading"><span>Diff</span><span className="subtle">{diff.trim() ? "unstaged" : "none"}</span></div>
              {!diff.trim() && <p className="git-clean git-diff-empty">No tracked file changes to show.</p>}
              {diff.trim() && <pre className="diff-view">{diff.split("\n").map((line, index) => <span key={index} className={line.startsWith("+") ? "added" : line.startsWith("-") ? "removed" : line.startsWith("@@") ? "hunk" : ""}>{line}{"\n"}</span>)}</pre>}
            </section>}
          </div>}

          {activity === "sessions" && <div className="panel-scroll session-list">
            {sessionState === "loading" && <LoadingRows />}
            {sessionState === "error" && <InlineError text={sessionError} retry={() => void loadSessions()} />}
            {sessionState === "ready" && sessions.length === 0 && <EmptyState title="No sessions yet" detail="Start a new session or send your first prompt." action={<button className="secondary-button" onClick={() => void createSession()}>New session</button>} />}
            {sessions.map((session) => <button key={session.id} className={`session-row ${activeSessionId === session.id ? "selected" : ""}`} onClick={() => selectSession(session)} disabled={streaming && activeSessionId !== session.id}>
              <span className="session-title">{session.title}</span><span className="session-meta">{modeForSession(session.mode).replace("_", " ")}{session.workspace ? ` · ${session.workspace}` : ""}</span>
            </button>)}
          </div>}

          {activity === "providers" && <div className="panel-scroll provider-panel">
            <div className="provider-card"><div className="provider-card-head"><span className="provider-glyph"><Icon name="spark" size={15} /></span><div><strong>{providerLabel}</strong><span>{isDesktop ? "Rust runtime · local-only" : "Vite preview · local-only"}</span></div><span className="status-dot" /></div><p>{isDesktop ? "The native shell uses the deterministic provider through the Rust runtime. No credentials or network requests are involved." : "The browser preview uses a deterministic local adapter. Native workspace access is available only through the Tauri shell."}</p></div>
            <div className="provider-note"><strong>Live providers are not connected</strong><p>Codex, Claude, and other hosted providers will appear only after secure authentication and runtime adapters are implemented. This surface does not claim account access.</p></div>
          </div>}
        </aside>

        <main className="main-pane">
          {selectedFile ? <>
            <div className="editor-tabs"><div className="editor-tab active"><span className={`file-icon ${fileKind(selectedFile)}`}><Icon name="file" size={13} /></span><span>{selectedFile.split("/").at(-1)}</span>{dirty && <span className="dirty-dot" title="Unsaved changes" />}</div><div className="editor-actions"><span className="editor-path">{selectedFile}</span><button className="save-button" onClick={() => void saveFile()} disabled={!dirty || saving || mode === "plan"} title={mode === "plan" ? "Saving is disabled in Plan mode" : "Save file"}><Icon name="save" size={14} />{saving ? "Saving…" : "Save"}</button></div></div>
            <div className="editor-wrap">
              {fileLoading && <div className="editor-state"><span className="spinner" />Loading file…</div>}
              {!fileLoading && fileError && <div className="editor-state error"><Icon name="alert" />{fileError}</div>}
              {!fileLoading && !fileError && <textarea ref={editorRef} className="code-editor" value={fileContent} onChange={(event) => setFileContent(event.target.value)} spellCheck={false} aria-label={`Editor for ${selectedFile}`} />}
            </div>
          </> : <Welcome workspace={workspace} openFiles={() => setActivity("files")} newSession={() => void createSession()} />}

          <section className={`output-pane ${outputOpen ? "open" : "closed"}`} aria-label="Output">
            <button className="output-heading" onClick={() => setOutputOpen((value) => !value)} aria-expanded={outputOpen}><Icon name="chevron" size={12} /><span>Output</span><span className="output-count">{output.length}</span><span className="output-runtime"><span className="status-dot" />{runtimeLabel}</span></button>
            {outputOpen && <div className="output-body" role="log" aria-live="polite">
              {output.length === 0 ? <span className="output-empty">Workspace and agent events will appear here.</span> : output.map((item) => <div key={item.id} className={`output-line ${item.kind}`}><span>{item.kind === "success" ? "✓" : item.kind === "error" ? "!" : item.kind === "tool" ? "›" : "·"}</span><span>{item.text}</span></div>)}
            </div>}
          </section>
        </main>

        <aside id="agent-panel" className={`agent-panel ${mobileAgentOpen ? "mobile-open" : ""}`} aria-label="Agent">
          <div className="agent-header"><div><span className="eyebrow">Agent</span><strong>{activeSession?.title ?? "New session"}</strong></div><button className="icon-button" onClick={() => void createSession()} disabled={streaming} aria-label="New session" title="New session"><Icon name="plus" size={15} /></button></div>
          <div className="provider-strip"><span className="status-dot" /><span>{providerLabel}</span><button onClick={() => setActivity("providers")}>Details</button></div>
          <div className="messages" aria-live="polite">
            {messagesLoading && <LoadingRows count={3} />}
            {!messagesLoading && messages.length === 0 && <div className="agent-empty"><span className="agent-orb"><Icon name="spark" size={21} /></span><h2>What should we work on?</h2><p>Ask about the workspace, plan a change, or try an edit flow with the {isDesktop ? "offline provider" : "local demo"}.</p><div className="suggestions"><button onClick={() => setPrompt("Explain this workspace")}>Explain this workspace</button><button onClick={() => setPrompt("Plan a focused improvement")}>Plan an improvement</button></div></div>}
            {!messagesLoading && messages.map((message, index) => <article key={index} className={`message ${message.role}`}><div className="message-role">{message.role === "user" ? "You" : "Agent"}</div><div className="message-content">{message.content || (streaming && index === messages.length - 1 ? <span className="thinking"><i /><i /><i /></span> : "")}</div></article>)}
            {permissionOperation && <div className="permission-notice"><span className="permission-icon"><Icon name="alert" size={14} /></span><div><strong>Permission required</strong><span>The host requested <code>{permissionOperation}</code>. Retry in Auto Edit mode if you want this operation enabled.</span></div></div>}
            {turnError && <div className="turn-error"><Icon name="alert" size={14} />{turnError}</div>}
            <div ref={messageEndRef} />
          </div>
          <form className="composer" onSubmit={submitPrompt}>
            <textarea ref={textareaRef} value={prompt} onChange={(event) => setPrompt(event.target.value)} onKeyDown={handlePromptKeyDown} placeholder="Ask the agent…" rows={3} disabled={streaming} aria-label="Agent prompt" />
            <div className="composer-toolbar">
              <label className="mode-select"><span className="sr-only">Permission mode</span><select value={mode} onChange={(event) => setMode(event.target.value as Exclude<PermissionMode, "autonomous">)} disabled={streaming}>{Object.entries(MODE_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
              <span className="composer-hint">Enter to send</span>
              {streaming ? <button type="button" className="send-button stop" onClick={cancel} aria-label="Cancel turn" title="Cancel turn"><Icon name="stop" size={13} /></button> : <button type="submit" className="send-button" disabled={!prompt.trim()} aria-label="Send prompt" title="Send prompt"><Icon name="send" size={14} /></button>}
            </div>
          </form>
        </aside>
      </div>

      <footer className="statusbar"><span><Icon name="git" size={12} />{workspace ? "workspace" : "no workspace"}</span><span className="statusbar-center">{mode === "plan" ? "Read-only planning" : mode === "ask" ? "Ask before edits" : "Workspace edits allowed"}</span><span>{runtimeLabel}</span></footer>

      {saveApproval && <div className="modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) { setSaveApproval(false); requestAnimationFrame(() => (saveFocusRef.current ?? editorRef.current)?.focus()); } }}>
        <div className="modal" role="dialog" aria-modal="true" aria-labelledby="save-title">
          <div className="modal-icon"><Icon name="save" size={18} /></div><h2 id="save-title">Approve file write?</h2><p>Ask mode requires confirmation before writing <strong>{selectedFile}</strong>.</p>
          <div className="modal-note">This writes the current editor content to your workspace.</div>
          <div className="modal-actions"><button ref={saveCancelRef} className="secondary-button" onClick={() => { setSaveApproval(false); requestAnimationFrame(() => (saveFocusRef.current ?? editorRef.current)?.focus()); }}>Cancel</button><button className="primary-button" onClick={() => void saveFile(true)} disabled={saving}>{saving ? "Saving…" : "Approve & save"}</button></div>
        </div>
      </div>}
    </div>
  );
}

function Welcome({ workspace, openFiles, newSession }: { workspace: { name: string; path: string } | null; openFiles: () => void; newSession: () => void }) {
  return <div className="welcome"><div className="welcome-content"><div className="welcome-mark"><Icon name="spark" size={23} /></div><p className="welcome-kicker">Universal agent harness</p><h1>{workspace ? `Ready in ${workspace.name}` : "Build with an agent at your side"}</h1><p className="welcome-copy">A focused desktop surface for workspace context, session history, approvals, and observable agent events.</p><div className="welcome-actions"><button className="primary-button" onClick={workspace ? newSession : openFiles}>{workspace ? "Start a session" : "Open a workspace"}</button><button className="secondary-button" onClick={openFiles}>Browse files</button></div><div className="feature-grid"><div><Icon name="files" /><strong>Workspace aware</strong><span>Browse, inspect, and edit plain-text files.</span></div><div><Icon name="sessions" /><strong>Resumable sessions</strong><span>Return to prior context and messages.</span></div><div><Icon name="check" /><strong>Explicit control</strong><span>Choose how edits are approved per session.</span></div></div></div></div>;
}

function EmptyState({ title, detail, action }: { title: string; detail: string; action?: ReactNode }) {
  return <div className="empty-state"><strong>{title}</strong><span>{detail}</span>{action}</div>;
}

function InlineError({ text, retry }: { text: string; retry: () => void }) {
  return <div className="inline-error"><Icon name="alert" size={16} /><strong>Something went wrong</strong><span>{text}</span><button onClick={retry}>Try again</button></div>;
}

function LoadingRows({ count = 5 }: { count?: number }) {
  return <div className="loading-rows" aria-label="Loading">{Array.from({ length: count }, (_, index) => <span key={index} style={{ width: `${72 - index * 6}%` }} />)}</div>;
}
