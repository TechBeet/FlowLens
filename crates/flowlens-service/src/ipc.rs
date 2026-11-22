//! IPC server for service control and queries.

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tracing::{error, info};

use flowlens_storage::StorageEngine;
use crate::service::ServiceState;

pub async fn run_ipc_server(state: ServiceState) -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:17890").await?;
    info!("IPC server listening on 127.0.0.1:17890");

    loop {
        let (socket, addr) = listener.accept().await?;
        let state = state.clone_for_ipc();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket, state).await {
                error!("IPC connection {} error: {}", addr, e);
            }
        });
    }
}

async fn handle_connection(
    socket: tokio::net::TcpStream,
    state: ServiceState,
) -> anyhow::Result<()> {
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let req: serde_json::Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(e) => {
                let resp = serde_json::json!({"error": format!("parse error: {}", e)});
                writer.write_all(format!("{}\n", resp).as_bytes()).await?;
                continue;
            }
        };

        let cmd = req.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let resp = match cmd {
            "start_capture" => {
                match state.start_capture().await {
                    Ok(()) => serde_json::json!({"ok": true}),
                    Err(e) => serde_json::json!({"error": e.to_string()}),
                }
            }
            "stop_capture" => {
                match state.stop_capture().await {
                    Ok(()) => serde_json::json!({"ok": true}),
                    Err(e) => serde_json::json!({"error": e.to_string()}),
                }
            }
            "get_status" => {
                let running = state.is_capturing().await;
                serde_json::json!({"capturing": running})
            }
            "list_sessions" => {
                match state.storage.list_sessions() {
                    Ok(sessions) => serde_json::json!({"sessions": sessions}),
                    Err(e) => serde_json::json!({"error": e.to_string()}),
                }
            }
            _ => serde_json::json!({"error": format!("unknown command: {}", cmd)}),
        };

        writer
            .write_all(format!("{}\n", resp).as_bytes())
            .await?;
    }
    Ok(())
}