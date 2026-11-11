//! WebSocket: broadcast service events to connected clients.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::state::SharedState;

pub async fn ws_handler(State(state): State<SharedState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();
    let mut events: broadcast::Receiver<krello_core::events::SharedEvent> = state.service.subscribe();

    // Forward service events to the client.
    let send_loop = async move {
        loop {
            match events.recv().await {
                Ok(event) => {
                    if sender.send(Message::text(event.json.as_ref())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("ws client lagged, {n} events dropped");
                    // Ask the client to resync.
                    let _ = sender
                        .send(Message::text(r#"{"type":"resync","data":null}"#))
                        .await;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    // Drain incoming messages (pings are answered by axum; we ignore data).
    let recv_loop = async move {
        while let Some(msg) = receiver.next().await {
            if msg.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = send_loop => {},
        _ = recv_loop => {},
    }
}
