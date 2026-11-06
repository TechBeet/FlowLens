// FlowLens TypeScript Types
// 这些类型与 crates/flowlens-core/src/models.rs 中的 TrafficEvent/TrafficRow 结构对应
export interface UiEvent {
  id: string;
  time: string;
  kind: string;                    // "connection" | "http" | "tls" | "ws" | "sse" | "packet" | "grpc" | "dropped" | "closed"
  info: string;
  size: number;
  src_addr?: string;
  dst_addr?: string;
  method?: string;
  url?: string;
  status?: string;
  // Extended fields (optional, backward compatible)
  session_id?: number;
  protocol?: "http1" | "http2" | "websocket" | "sse" | "grpc" | "unknown";
  process_name?: string;
  pid?: number;
  match_quality?: "exact" | "heuristic" | "unspecified";
  tls_version?: string;
  sni?: string;
  cipher_suite?: string;
  requestHeaders?: Record<string, string>;
  responseHeaders?: Record<string, string>;
  requestBody?: string;
  responseBody?: string;
  paused?: boolean;
  rewrite_applied?: string[];
  timestamp_micros?: number;
}

export interface TrafficEvent extends UiEvent {
  // Full session detail; may include body blobs (base64)
}

export interface TrafficStats {
  connections: number;
  http: number;
  tls: number;
  sse: number;
  websocket: number;
  total: number;
}

export interface FilterState {
  host: string;
  pid: string;
  method: string;
  status: string;
}
