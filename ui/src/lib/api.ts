// FlowLens Tauri API bindings + theme management
import { invoke } from "@tauri-apps/api/core";
import type { TrafficStats } from "./types";
import type { ShortcutMap } from "./shortcuts";
import type { RewriteRule } from "./rewriteRules";

export type { RewriteRule, RewriteAction } from "./rewriteRules";

// ---------- Tauri commands ----------

/** Capture mode. `pcap` requires Npcap; `windivert` is transparent (no proxy). */
export type CaptureMode = "pcap" | "windivert";

export async function startCapture(mode?: CaptureMode, device?: string): Promise<void> {
  await invoke("start_capture", {
    ...(mode ? { mode } : {}),
    ...(device ? { device } : {}),
  });
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

/// Per-entry options controlling what the HAR 1.2 exporter emits.
/// All three are optional and default to `true` on the Rust side.
export interface HarExportOptions {
  include_headers: boolean;
  include_bodies: boolean;
  include_timing: boolean;
}

export async function exportHAR(
  path: string,
  options?: Partial<HarExportOptions>,
): Promise<void> {
  await invoke("export_har", { path, options });
}

/// Open the native "Save File" dialog and return the chosen absolute path,
/// or `null` when the user cancels.
export async function pickSavePath(defaultName = "trace.har"): Promise<string | null> {
  return await invoke<string | null>("pick_save_path", { defaultName });
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

export async function addRewriteRule(rule: Partial<RewriteRule>): Promise<void> {
  await invoke("add_rewrite_rule", { rule });
}

export async function removeRewriteRule(id: string): Promise<boolean> {
  return (await invoke("remove_rewrite_rule", { id })) as boolean;
}

export async function toggleRewriteRule(id: string, enabled: boolean): Promise<boolean> {
  return (await invoke("toggle_rewrite_rule", { id, enabled })) as boolean;
}

export async function listRewriteRules(): Promise<RewriteRule[]> {
  return (await invoke("list_rewrite_rules")) as RewriteRule[];
}

// ---------- Replay & compare ----------

export async function replayRequest(sessionId: number): Promise<unknown> {
  return await invoke("replay_request", { session_id: sessionId });
}

/// Replay a session after an optional delay with custom header overrides.
///
/// `delayMs` is applied in the webview so that batch replay can space out
/// successive requests without needing a separate backend command.
/// `headers` is forwarded to the backend (which may ignore it until the
/// command grows header support), so callers never have to branch.
export async function replayRequestWithDelay(
  sessionId: number,
  delayMs: number,
  headers: Record<string, string> = {}
): Promise<unknown> {
  if (delayMs > 0) {
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }
  return await invoke("replay_request", { session_id: sessionId, headers });
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

/// Per-host decryption policy endpoints (SNI-aware MITM certificate management).

export interface UiDecryptEntry {
  domain: string;
  decrypt: boolean;
  wildcard: boolean;
  cert_ready?: boolean;
  added_at?: number;
}

export interface SniCertInfo {
  sni: string;
  subject_cn: string;
  fingerprint_sha256: string;
  cert_pem: string;
  key_pem: string;
  sans: string[];
}

export async function listDecryptDomains(): Promise<UiDecryptEntry[]> {
  return (await invoke("list_decrypt_domains")) as UiDecryptEntry[];
}

export async function addDecryptDomain(domain: string): Promise<UiDecryptEntry> {
  return (await invoke("add_decrypt_domain", { domain })) as UiDecryptEntry;
}

export async function removeDecryptDomain(domain: string): Promise<boolean> {
  return (await invoke("remove_decrypt_domain", { domain })) as boolean;
}

export async function toggleDecryptDomain(
  domain: string,
  enable: boolean
): Promise<UiDecryptEntry> {
  return (await invoke("toggle_decrypt_domain", { domain, enable })) as UiDecryptEntry;
}

export async function issueForSni(sni: string): Promise<SniCertInfo> {
  return (await invoke("issue_for_sni", { sni })) as SniCertInfo;
}

// ---------- Theme management ----------

export type Theme = "dark" | "light";

export async function loadTheme(): Promise<Theme> {
  try {
    const saved = await invoke<string>("read_theme");
    if (saved === "dark" || saved === "light") return saved;
  } catch {
    // Fall through to default
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

export async function saveTheme(theme: Theme): Promise<void> {
  try {
    await invoke("write_theme", { theme });
  } catch {
    // Persistence is best-effort only
  }
}

export function toggleTheme(current: Theme): Theme {
  return current === "dark" ? "light" : "dark";
}

// ---------- Keyboard shortcut persistence ----------

/// Load persisted shortcut overrides. Returns an empty map when nothing is
/// stored (or the backend is unavailable), so callers fall back to defaults.
export async function loadShortcuts(): Promise<ShortcutMap> {
  try {
    const saved = await invoke<ShortcutMap | null>("read_shortcuts");
    if (saved && typeof saved === "object") return saved;
  } catch {
    // Persistence is best-effort only
  }
  return {};
}

/// Persist shortcut overrides. Never throws: a failure must not break the UI.
export async function saveShortcuts(shortcuts: ShortcutMap): Promise<void> {
  try {
    await invoke("write_shortcuts", { shortcuts });
  } catch {
    // Persistence is best-effort only
  }
}

/// Set the maximum number of events to retain in the ring buffer.
/// Persists the cap via the Tauri command; the eventStore will read this file on init.
export async function setEventMax(cap: number): Promise<void> {
  await invoke<void>("set_event_max", { cap });
}

/// Get the maximum number of events from event_max.txt.
/// Returns the stored cap or default 10000 if file doesn't exist or on error.
export async function getEventMax(): Promise<number> {
  try {
    return await invoke<number>("get_event_max");
  } catch (error) {
    console.warn("Failed to get event max:", error);
    return 10000;
  }
}

/// Return the last N lines of the flowlens log file (for self-diagnostics).
export async function getRecentLogs(lines = 200): Promise<string> {
  try {
    return await invoke<string>("get_recent_logs", { lines });
  } catch {
    return "(日志不可用)";
  }
}

/// Return structured diagnostics snapshot (stats, counts, log path).
export async function getDiagnostics(): Promise<Record<string, unknown>> {
  try {
    return await invoke<Record<string, unknown>>("get_diagnostics");
  } catch {
    return {};
  }
}

/// Persisted application settings (capture backend, theme, rules, filter profiles).
export interface AppSettings {
  capture: { backend: string; filter: string; auto_start: boolean };
  highlight_rules: unknown[];
  responder_rules: unknown[];
  filter_profiles: unknown[];
  theme: string;
}

export async function loadSettings(): Promise<AppSettings> {
  return (await invoke<AppSettings>("load_settings")) as AppSettings;
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  await invoke("save_settings", { settings });
}
