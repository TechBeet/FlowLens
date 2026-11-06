// FlowLens Tauri API bindings + theme management
import { invoke } from "@tauri-apps/api/core";
import type { TrafficStats } from "./types";

// ---------- Tauri commands ----------

export async function startCapture(device?: string): Promise<void> {
  await invoke("start_capture", device ? { device } : {});
}

export async function stopCapture(): Promise<void> {
  await invoke("stop_capture");
}

export async function getStats(): Promise<TrafficStats> {
  return (await invoke("get_stats")) as TrafficStats;
}

export async function clearEvents(): Promise<void> {
  await invoke("clear_events");
}

export async function listDevices(): Promise<string[]> {
  return (await invoke("list_devices")) as string[];
}

export async function exportHAR(path: string): Promise<void> {
  await invoke("export_har", { path });
}

// ---------- Session interception ----------

export async function pauseSession(sessionId: number): Promise<void> {
  await invoke("pause_session", { session_id: sessionId });
}

export async function releaseSession(sessionId: number): Promise<void> {
  await invoke("release_session", { session_id: sessionId });
}

export async function modifySession(
  sessionId: number,
  newData: Record<string, unknown>
): Promise<void> {
  await invoke("modify_session", { session_id: sessionId, new_data: newData });
}

export async function listPausedSessions(): Promise<unknown> {
  return await invoke("list_paused_sessions");
}

// ---------- Rewrite rules ----------

export async function addRewriteRule(rule: Record<string, unknown>): Promise<void> {
  await invoke("add_rewrite_rule", { rule });
}

export async function removeRewriteRule(id: string): Promise<boolean> {
  return (await invoke("remove_rewrite_rule", { id })) as boolean;
}

export async function toggleRewriteRule(id: string, enabled: boolean): Promise<boolean> {
  return (await invoke("toggle_rewrite_rule", { id, enabled })) as boolean;
}

export async function listRewriteRules(): Promise<unknown[]> {
  return (await invoke("list_rewrite_rules")) as unknown[];
}

// ---------- Replay & compare ----------

export async function replayRequest(sessionId: number): Promise<unknown> {
  return await invoke("replay_request", { session_id: sessionId });
}

export async function compareSessions(idA: number, idB: number): Promise<unknown> {
  return await invoke("compare_sessions", { id_a: idA, id_b: idB });
}

// ---------- CA management ----------

export interface CaStatus {
  installed: boolean;
  cn: string | null;
  pem: string | null;
}

export async function installCA(cn: string): Promise<void> {
  await invoke("install_ca", { cn });
}

export async function uninstallCA(cn: string): Promise<void> {
  await invoke("uninstall_ca", { cn });
}

export async function checkCAStatus(cn: string): Promise<CaStatus> {
  return (await invoke("check_ca_status", { cn })) as CaStatus;
}

export async function getCAPem(): Promise<string | null> {
  return (await invoke("get_ca_pem")) as string | null;
}

export async function getCAInfo(): Promise<unknown> {
  return await invoke("get_ca_info");
}

export async function exportCAQR(): Promise<string> {
  return (await invoke("export_ca_qr")) as string;
}

export async function autoReleasePaused(sessionId: number, timeoutMs: number): Promise<void> {
  await invoke("auto_release_paused", { session_id: sessionId, timeout_ms: timeoutMs });
}

export async function generateCA(cn: string): Promise<string> {
  return (await invoke("generate_ca", { cn })) as string;
}

export async function getSessionDetail(sessionId: number): Promise<unknown> {
  return await invoke("get_session_detail", { session_id: sessionId });
}

export async function querySessions(query: Record<string, unknown>): Promise<unknown> {
  return await invoke("query_sessions", { query });
}

export async function deleteSessions(before?: number): Promise<number> {
  return (await invoke("delete_sessions", { before })) as number;
}

// ---------- Theme management ----------

export type Theme = "dark" | "light";

const STORAGE_KEY = "flowlens-theme";

export function loadTheme(): Theme {
  if (typeof window === "undefined") return "dark";
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved === "dark" || saved === "light") return saved;
  } catch {
    // localStorage may be unavailable (private mode) - fall through to default
  }
  // Fall back to the OS preference
  if (typeof window !== "undefined" && window.matchMedia) {
    try {
      if (window.matchMedia("(prefers-color-scheme: light)").matches) return "light";
    } catch {
      // matchMedia can throw in some embedded webviews
    }
  }
  return "dark";
}

export function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", theme);
}

export function saveTheme(theme: Theme): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Persistence is best-effort only
  }
}

export function toggleTheme(current: Theme): Theme {
  return current === "dark" ? "light" : "dark";
}
