//! WebSocket 连接处理

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use sqlx::Pool;
use sqlx::Sqlite;
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

use crate::core::event::{Event, EventBus};
use crate::models::{WsMessage, TaskUpdateData, TaskProgressData, ChannelStatusData, SystemAlertData};

/// 将内部事件转换为 WebSocket 消息
fn event_to_ws_message(event: Event) -> WsMessage {
    match event {
        Event::TaskUpdate(e) => WsMessage::TaskUpdate(TaskUpdateData {
            task_id: e.task_id,
            status: e.status.to_string(),
            error_message: e.error_message,
        }),
        Event::TaskProgress(e) => WsMessage::TaskProgress(TaskProgressData {
            task_id: e.task_id,
            percent: e.percent,
            downloaded_bytes: e.downloaded_bytes,
            speed: e.speed,
            eta_seconds: e.eta_seconds,
        }),
        Event::ChannelStatus(e) => WsMessage::ChannelStatus(ChannelStatusData {
            channel_id: e.channel_id,
            status: e.status.to_string(),
        }),
        Event::SystemAlert(e) => WsMessage::SystemAlert(SystemAlertData {
            level: e.level.to_string(),
            message: e.message,
            details: e.details,
        }),
    }
}

/// 处理 WebSocket 连接
pub async fn handle_socket(
    socket: WebSocket,
    _db: Pool<Sqlite>,
    event_bus: Arc<EventBus>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // 发送欢迎消息
    let welcome = serde_json::json!({
        "type": "connected",
        "data": {
            "message": "WebSocket connection established",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    })
    .to_string();

    if let Err(e) = ws_sender.send(Message::Text(welcome.into())).await {
        tracing::error!("Failed to send welcome message: {}", e);
        return;
    }

    let mut event_rx = event_bus.subscribe();

    loop {
        tokio::select! {
            // 转发事件总线消息到客户端
            event_result = event_rx.recv() => {
                match event_result {
                    Ok(event) => {
                        let ws_msg = event_to_ws_message(event);
                        match serde_json::to_string(&ws_msg) {
                            Ok(json) => {
                                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to serialize WS message: {}", e);
                            }
                        }
                    }
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!("WebSocket event receiver lagged by {} messages", n);
                    }
                }
            }

            // 处理来自客户端的消息
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            tracing::debug!("Received WS message: {}", json);

                            // 处理 ping
                            if json["type"] == "ping" {
                                let pong = serde_json::json!({
                                    "type": "pong",
                                    "data": {
                                        "timestamp": chrono::Utc::now().to_rfc3339()
                                    }
                                })
                                .to_string();
                                if ws_sender.send(Message::Text(pong.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::info!("Client disconnected");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::info!("WebSocket connection closed");
}
