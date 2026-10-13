export type PermissionMode = "plan" | "ask" | "auto_edit" | "autonomous";

export type Session = {
  id: string;
  title: string;
  workspace: string;
  mode: PermissionMode;
};

export type Message = { role: string; content: string };

export type AgentEvent =
  | { type: "text_delta"; text: string }
  | { type: "tool_started"; name: string }
  | { type: "tool_finished"; name: string; output: string }
  | { type: "permission_required"; operation: string }
  | { type: "finished" }
  | { type: "cancelled" }
  | { type: "error"; message: string };
