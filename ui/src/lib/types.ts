// FlowLens TypeScript Types
// 这些类型与 crates/flowlens-core/src/models.rs 中的 TrafficEvent/TrafficRow 结构对应
export interface TrafficEvent {
  id: string;
  time: string;
  kind: string;  // "connection"/"http"/"tls"/"ws"/"sse"/"packet"/...
  info: string;  // 描述性文本
  size: number;  // 字节大小（对应 body_size / payload_size）
  src_addr?: string;
  dst_addr?: string;
  method?: string;
  url?: string;
  status?: string;
  requestHeaders?: Record<string, string>;
  responseHeaders?: Record<string, string>;
  requestBody?: string;
  responseBody?: string;
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
