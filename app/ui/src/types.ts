// Mirrors of the daemon's serde types (app/src-tauri/src/state.rs and
// registry.rs). The Rust side is authoritative; when the two disagree,
// this file is the bug.

export type SessionState =
  | "idle"
  | "thinking"
  | "needs_input"
  | "complete"
  | "error"
  | "ended"
  | "unknown";

export type InputKind = "permission" | "question";

export interface OpenOp {
  id: string | null;
  tool: string;
  openedAtMs: number;
}

export interface ErrorDetail {
  kind: string;
  message: string | null;
}

export interface SessionSnap {
  id: string;
  label: string;
  cwd: string | null;
  permissionMode: string | null;
  state: SessionState;
  stateSinceMs: number;
  detailKind: InputKind | null;
  detailTool: string | null;
  question: string | null;
  options: string[];
  error: ErrorDetail | null;
  children: number;
  openOps: OpenOp[];
  lastEventAtMs: number;
  unreadSinceMs: number | null;
  pendingComplete: boolean;
}

export interface TileSnapshot {
  index: number;
  selected: boolean;
  session: SessionSnap | null;
}

export interface Snapshot {
  tiles: TileSnapshot[];
  nowMs: number;
}

export interface BindableSession {
  id: string;
  label: string;
  cwd: string | null;
  state: SessionState;
  boundTo: number | null;
}

// The pieces of the injected Tauri global this surface uses.
export interface TauriApi {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> };
  event: {
    listen<T>(
      name: string,
      handler: (e: { payload: T }) => void
    ): Promise<() => void>;
  };
}

export function tauri(): TauriApi {
  return (window as unknown as { __TAURI__: TauriApi }).__TAURI__;
}
