//! The task that owns the browser's WebSocket: it writes the commands it is handed and hands every message the browser
//! sends to the routes, until either side closes.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

use super::Routes;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Serves `socket` on a task of its own; the commands to write go into the returned sender, and dropping it closes the
/// socket.
pub(super) fn serve(socket: Socket, routes: Arc<Routes>) -> mpsc::UnboundedSender<String> {
  let (outgoing, commands) = mpsc::unbounded_channel();
  tokio::spawn(async move {
    exchange(socket, commands, &routes).await;
    routes.close();
  });
  outgoing
}

async fn exchange(mut socket: Socket, mut commands: mpsc::UnboundedReceiver<String>, routes: &Routes) {
  loop {
    tokio::select! {
      command = commands.recv() => {
        let Some(command) = command else {
          if let Err(error) = socket.close(None).await {
            tracing::debug!(%error, "the browser's connection did not close cleanly");
          }
          return;
        };
        if let Err(error) = socket.send(Message::text(command)).await {
          tracing::warn!(%error, "cannot send a command to the browser");
          return;
        }
      }
      message = socket.next() => match message {
        Some(Ok(Message::Text(text))) => routes.deliver(text.as_str()),
        Some(Ok(Message::Close(_))) | None => return,
        // Pings are answered by the socket itself; the protocol sends nothing else.
        Some(Ok(_)) => {}
        Some(Err(error)) => {
          tracing::warn!(%error, "the connection to the browser failed");
          return;
        }
      },
    }
  }
}
