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
  heard: boolean;
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
  hideUnknown: boolean;
}

// Mirrors of the daemon's settings-panel types
// (app/src-tauri/src/runkey.rs, hook_status.rs, installer.rs). As with
// SessionState above, the Rust side is authoritative; its serialization
// tests (runkey.rs's serializes_as_the_wire_shape_the_surface_expects
// and the sibling tests in hook_status.rs and installer.rs) pin the
// exact shapes these types describe.

export type StartWithWindowsState =
  | { kind: "off" }
  | { kind: "onThisExe" }
  | { kind: "onOtherExe"; path: string };

export type HookStatus = "installed" | "outdated" | "missing" | "unreadable";

export type RepairOutcome = "ran" | "timed_out" | "failed_to_start";

export interface SettingsSnapshot {
  alwaysOnTop: boolean;
  startWithWindows: StartWithWindowsState;
  hookStatus: HookStatus;
  installerAvailable: boolean;
}

export interface RepairResult {
  outcome: RepairOutcome;
  hookStatus: HookStatus;
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
