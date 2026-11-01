// FlowLens Tauri API bindings + theme management
import { invoke } from "@tauri-apps/api/core";
import type { TrafficStats } from "./types";

// ---------- Tauri commands ----------

export async function startCapture(): Promise<void> {
  await invoke("start_capture");
}

export async function stopCapture(): Promise<void> {
  await invoke("stop_capture");
}

export async function getStats(): Promise<TrafficStats> {
  return (await invoke("get_stats")) as TrafficStats;
}

export async function listDevices(): Promise<string[]> {
  return (await invoke("list_devices")) as string[];
}

export async function exportHAR(path: string): Promise<void> {
  await invoke("export_har", { path });
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
