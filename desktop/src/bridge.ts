import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentEvent, Message, PermissionMode, Session } from "./types";

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Workspace = { name: string; path: string };
type FileEntry = { path: string; name: string; is_dir: boolean };

type DemoState = {
  files: Record<string, string>;
  sessions: Session[];
  messages: Record<string, Message[]>;
};

const demoPath = "/demo/agent-harness";
const initialDemo: DemoState = {
  files: {
    "README.md": "# Agent Harness\n\nThis is a browser preview of the local-first agent harness. Open a file, edit it, and try a deterministic offline turn.\n",
    "src/main.rs": "fn main() {\n    println!(\"Welcome to Harness\");\n}\n",
    "docs/next-steps.md": "# Next steps\n\nConnect the native Tauri host to the Rust workspace tools and provider adapters.\n",
  },
  sessions: [],
  messages: {},
};

function loadDemo(): DemoState {
  try {
    const raw = sessionStorage.getItem("harness-demo-v1");
    if (raw) return JSON.parse(raw) as DemoState;
  } catch {
    // Browser storage may be disabled; keep a working in-memory preview.
  }
  return structuredClone(initialDemo);
}

const demo = loadDemo();
let workspace: Workspace | null = null;

function saveDemo() {
  try {
    sessionStorage.setItem("harness-demo-v1", JSON.stringify(demo));
  } catch {
    // In-memory operation still works for this tab.
  }
}

function requireDemoWorkspace() {
  if (!workspace) throw new Error("Open the demo workspace first.");
}

function assertDemoPath(path: string) {
  if (!Object.hasOwn(demo.files, path)) throw new Error(`File not found: ${path}`);
}

function ensureSession(sessionId: string): Message[] {
  const messages = demo.messages[sessionId];
  if (!messages) throw new Error("Session not found.");
  return messages;
}

export const bridge = {
  async openWorkspace(path: string): Promise<Workspace> {
    if (isDesktop) {
      workspace = await invoke<Workspace>("open_workspace", { path });
      return workspace;
    }
    if (path !== demoPath) throw new Error(`Browser preview only supports ${demoPath}. Use the desktop app for local folders.`);
    workspace = { name: "agent-harness", path: demoPath };
    return workspace;
  },

  async listFiles(): Promise<FileEntry[]> {
    if (isDesktop) return invoke<FileEntry[]>("list_files");
    requireDemoWorkspace();
    const directories = new Set<string>();
    for (const path of Object.keys(demo.files)) {
      const segments = path.split("/");
      for (let index = 1; index < segments.length; index++) directories.add(segments.slice(0, index).join("/"));
    }
    return [
      ...[...directories].map((path) => ({ path, name: path.split("/").at(-1)!, is_dir: true })),
      ...Object.keys(demo.files).map((path) => ({ path, name: path.split("/").at(-1)!, is_dir: false })),
    ].sort((a, b) => a.path.localeCompare(b.path));
  },

  async readFile(path: string): Promise<string> {
    if (isDesktop) return invoke<string>("read_file", { path });
    requireDemoWorkspace();
    assertDemoPath(path);
    return demo.files[path];
  },

  async writeFile(path: string, content: string, mode: PermissionMode, approved: boolean, expectedContent?: string): Promise<void> {
    if (isDesktop) return invoke<void>("write_file", { path, content, mode, approved, expectedContent });
    requireDemoWorkspace();
    assertDemoPath(path);
    if (mode === "plan") throw new Error("Plan mode does not allow file writes.");
    if (mode === "ask" && !approved) throw new Error("This file write requires approval.");
    if (expectedContent !== undefined && demo.files[path] !== expectedContent) throw new Error("File changed in the demo workspace since it was opened.");
    demo.files[path] = content;
    saveDemo();
  },

  async gitStatus(): Promise<string> {
    if (isDesktop) return invoke<string>("git_status");
    requireDemoWorkspace();
    const changed = Object.keys(demo.files).filter((path) => initialDemo.files[path] !== demo.files[path]);
    return changed.map((path) => ` M ${path}`).join("\n");
  },

  async searchText(query: string): Promise<string> {
    if (isDesktop) return invoke<string>("search_text", { query });
    requireDemoWorkspace();
    if (!query.trim()) throw new Error("Search query must not be empty.");
    const results: string[] = [];
    for (const [path, content] of Object.entries(demo.files)) {
      content.split("\n").forEach((line, index) => {
        if (line.includes(query)) results.push(`${path}:${index + 1}:${line}`);
      });
    }
    return results.join("\n");
  },

  async gitDiff(): Promise<string> {
    if (isDesktop) return invoke<string>("git_diff");
    requireDemoWorkspace();
    const changed = Object.entries(demo.files).filter(([path, text]) => initialDemo.files[path] !== text);
    return changed.length
      ? changed.map(([path]) => `Modified in browser preview: ${path}\nGit integration is available in the desktop app.`).join("\n\n")
      : "";
  },

  async listSessions(): Promise<Session[]> {
    if (isDesktop) return invoke<Session[]>("list_sessions");
    return [...demo.sessions];
  },

  async createSession(title: string, mode: PermissionMode): Promise<Session> {
    if (isDesktop) return invoke<Session>("create_session", { title, mode });
    requireDemoWorkspace();
    const session: Session = { id: crypto.randomUUID(), title, mode, workspace: workspace!.path };
    demo.sessions.unshift(session);
    demo.messages[session.id] = [];
    saveDemo();
    return session;
  },

  async getMessages(sessionId: string): Promise<Message[]> {
    if (isDesktop) return invoke<Message[]>("get_messages", { sessionId });
    return [...ensureSession(sessionId)];
  },

  async sendPrompt(
    sessionId: string,
    prompt: string,
    mode: PermissionMode,
    onEvent: (event: AgentEvent) => void,
  ): Promise<() => void> {
    if (isDesktop) {
      const turnId = crypto.randomUUID();
      const unlisten = await listen<{ turn_id: string; event: AgentEvent }>("agent-event", ({ payload }) => {
        if (payload.turn_id !== turnId) return;
        onEvent(payload.event);
        if (["finished", "cancelled", "error"].includes(payload.event.type)) unlisten();
      });
      try {
        await invoke<void>("start_offline_turn", { sessionId, prompt, mode, turnId });
      } catch (error) {
        unlisten();
        throw error;
      }
      return () => { void invoke<void>("cancel_turn", { turnId }).catch((error) => onEvent({ type: "error", message: String(error) })); };
    }

    const transcript = ensureSession(sessionId);
    transcript.push({ role: "user", content: prompt });
    saveDemo();
    const fragments = [
      "Offline demo mode: no model or network provider is connected.\n",
      `Received this prompt locally: ${JSON.stringify(prompt.slice(0, 240))}\n`,
      `Permission mode: ${mode}.\n`,
      "Native provider integration is planned. This browser preview exercises the streaming interface and session flow.",
    ];
    let index = 0;
    let content = "";
    let settled = false;
    const finish = (event: AgentEvent) => {
      if (settled) return;
      settled = true;
      if (content) transcript.push({ role: "assistant", content });
      saveDemo();
      onEvent(event);
    };
    const timer = window.setInterval(() => {
      if (index >= fragments.length) {
        window.clearInterval(timer);
        finish({ type: "finished" });
        return;
      }
      const text = fragments[index++];
      content += text;
      onEvent({ type: "text_delta", text });
    }, 45);
    return () => {
      window.clearInterval(timer);
      finish({ type: "cancelled" });
    };
  },
};
