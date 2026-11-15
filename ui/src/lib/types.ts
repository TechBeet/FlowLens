// FlowLens TypeScript Types
// Hand-written types for the Tauri `UiEvent` wire shape (defined in
// ui/src-tauri/src/commands.rs). Generated types live in `./generated/` and are
// produced from the Rust structs via ts-rs (`cargo clippy --features typescript`).
//
// Type notes (verified against Rust structs):
// - UiEvent.size: Rust usize -> number (JS-safe)
// - UiEvent.pid: Rust Option<u32> -> number | undefined
// - UiEvent.paused / rewrite_applied: Rust bool -> boolean
// - UiEvent.protocol / session_id: Rust Option<String>/SessionId(bigint) -> string | undefined / bigint | undefined
// - TrafficStats: Rust u64 counters serialise as JSON numbers; use JS `number`
// Re-export the generated Rust types so consumers can import from `types`.
export type {
  TrafficFilter,
  TrafficRow,
  ProcessInfo,
  ProcessArch,
  MatchQuality,
  ProtocolKind,
  TransportProtocol,
  SessionId,
} from "./generated";

import type { ProcessInfo as GeneratedProcessInfo, MatchQuality, SessionId } from "./generated";

export interface UiEvent {
  id: string;
  time: string;
  kind: string; // "connection" | "http" | "tls" | "ws" | "sse" | "packet" | "grpc" | "dropped" | "closed"
  info: string;
  size: number;
  src_addr?: string;
  dst_addr?: string;
  method?: string;
  url?: string;
  status?: string;
  // Extended fields (optional, backward compatible)
  session_id?: SessionId; // bigint (Rust SessionId)
  protocol?: string; // Rust Option<String>
  process_name?: string;
  pid?: number; // Rust Option<u32>
  match_quality?: MatchQuality;
  tls_version?: string; // Rust Option<String>
  sni?: string; // Rust Option<String>
  cipher_suite?: string; // Rust Option<String>
  requestHeaders?: Record<string, string>;
  responseHeaders?: Record<string, string>;
  requestBody?: string;
  responseBody?: string;
  paused?: boolean; // Rust bool
  rewrite_applied?: boolean; // Rust bool (was incorrectly string[])
  timestamp_micros?: number;
}

export interface TrafficEvent extends UiEvent {
  // Full session detail; may include body blobs (base64)
}

export interface FilterState {
  host: string;
  pid: string;
  process: string;
  method: string;
  status: string;
}

// UI-facing stats use JS numbers (the backend serialises u64 counters).
export interface TrafficStats {
  connections: number;
  http: number;
  tls: number;
  sse: number;
  websocket: number;
  total: number;
}

/** Process attribution as emitted by the capture pipeline. */
export type UiProcessInfo = GeneratedProcessInfo;
