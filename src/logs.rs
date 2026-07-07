use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use tokio::io::AsyncBufReadExt;
use tracing::debug;

/// WebSocket upgrade handler for the docker logs endpoint.
/// Binds to `/ws/logs` and streams `docker logs -f --tail=100` for the
/// first detected vLLM/DeepSeek container.
pub async fn ws_logs_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_logs_socket)
}

async fn handle_logs_socket(mut socket: WebSocket) {
    debug!("Logs WebSocket client connected");

    // Discover the target container
    let container_id = match find_target_container().await {
        Some(id) => id,
        None => {
            let msg = "ERR:No vLLM/DeepSeek container found".to_string();
            let _ = socket.send(Message::Text(msg.into())).await;
            return;
        }
    };

    debug!("Streaming logs for container: {}", container_id);

    // Spawn `docker logs -f --tail=100` and stream its output
    let mut cmd = tokio::process::Command::new("docker")
        .args([
            "logs",
            "-f",
            "--tail=100",
            &container_id,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn();

    let mut child = match cmd {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("ERR:Failed to spawn docker logs: {}", e);
            let _ = socket.send(Message::Text(msg.into())).await;
            return;
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Read stdout line by line and send over WebSocket
    if let Some(stdout) = stdout {
        let reader = tokio::io::BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if socket.send(Message::Text(line.into())).await.is_err() {
                break;
            }
        }
    }

    // Read stderr line by line
    if let Some(stderr) = stderr {
        let reader = tokio::io::BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if socket
                .send(Message::Text(format!("[stderr] {}", line).into()))
                .await
                .is_err()
            {
                break;
            }
        }
    }

    debug!("Logs WebSocket client disconnected");
    let _ = child.wait().await;
}

/// Scan Docker for a running container whose image name contains "vllm" or
/// "deepseek". Returns the first match's container ID (short form).
async fn find_target_container() -> Option<String> {
    let output = tokio::process::Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.ID}}\t{{.Image}}",
            "--no-trunc",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let id = parts[0].trim();
        let image = parts[1].to_lowercase();
        if image.contains("vllm") || image.contains("deepseek") {
            // Return the short 12-char container ID
            let short_id: String = id.chars().take(12).collect();
            return Some(short_id);
        }
    }

    None
}
