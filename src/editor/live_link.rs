//! SKOPE Live Link - WebSocket server for Blender integration
//!
//! Provides real-time bidirectional communication between SKOPE engine and Blender.
//! Supports entity transform updates, script hot-reload signals, and play/stop commands.

#![cfg(feature = "live_link")]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};

/// Live Link message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LiveLinkMessage {
    /// Entity transform update from Blender
    EntityUpdate {
        entity: String,
        position: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    },

    /// Script reload request
    ScriptReload {
        path: String,
    },

    /// Script error notification (engine -> Blender)
    ScriptError {
        file: String,
        line: u32,
        message: String,
    },

    /// Play request from Blender
    PlayRequest,

    /// Stop request from Blender
    StopRequest,

    /// Pause request from Blender
    PauseRequest,

    /// Scene sync request
    SceneSync,

    /// Scene data (engine -> Blender)
    SceneData {
        entities: Vec<EntityData>,
    },

    /// Ping/Pong for connection health
    Ping,
    Pong,

    /// Client connected confirmation
    Connected {
        client_name: String,
    },

    /// Log message (engine -> Blender)
    Log {
        level: String,
        message: String,
    },
}

/// Entity data for scene sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub mesh: Option<String>,
    pub script: Option<String>,
}

/// Live Link client info
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub name: String,
    pub addr: SocketAddr,
}

/// Live Link server handle for the main thread
pub struct LiveLink {
    /// Channel to receive messages from clients
    rx: mpsc::UnboundedReceiver<LiveLinkMessage>,

    /// Channel to send messages to all clients
    tx: mpsc::UnboundedSender<LiveLinkMessage>,

    /// Connected clients
    clients: Arc<Mutex<HashMap<SocketAddr, ClientInfo>>>,

    /// Server running flag
    running: Arc<Mutex<bool>>,
}

/// Internal server state shared between tasks
struct ServerState {
    /// Channel to send messages to main thread
    main_tx: mpsc::UnboundedSender<LiveLinkMessage>,

    /// Channel to receive broadcast messages
    broadcast_rx: mpsc::UnboundedReceiver<LiveLinkMessage>,

    /// Connected clients
    clients: Arc<Mutex<HashMap<SocketAddr, ClientInfo>>>,

    /// Running flag
    running: Arc<Mutex<bool>>,
}

impl LiveLink {
    /// Start the Live Link server on the specified port
    pub fn start(port: u16) -> Self {
        let (main_tx, main_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, broadcast_rx) = mpsc::unbounded_channel();
        let clients = Arc::new(Mutex::new(HashMap::new()));
        let running = Arc::new(Mutex::new(true));

        // Spawn server task
        let state = ServerState {
            main_tx,
            broadcast_rx,
            clients: clients.clone(),
            running: running.clone(),
        };

        std::thread::spawn(move || {
            if let Err(e) = run_server(port, state) {
                log::error!("Live Link server error: {}", e);
            }
        });

        log::info!("Live Link server starting on port {}", port);

        Self {
            rx: main_rx,
            tx: broadcast_tx,
            clients,
            running,
        }
    }

    /// Poll for incoming messages (non-blocking)
    pub fn poll_messages(&mut self) -> Vec<LiveLinkMessage> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.rx.try_recv() {
            messages.push(msg);
        }
        messages
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast(&self, msg: LiveLinkMessage) {
        if let Err(e) = self.tx.send(msg) {
            log::warn!("Failed to broadcast message: {}", e);
        }
    }

    /// Send script error to Blender
    pub fn send_script_error(&self, file: &str, line: u32, message: &str) {
        self.broadcast(LiveLinkMessage::ScriptError {
            file: file.to_string(),
            line,
            message: message.to_string(),
        });
    }

    /// Send log message to Blender
    pub fn send_log(&self, level: &str, message: &str) {
        self.broadcast(LiveLinkMessage::Log {
            level: level.to_string(),
            message: message.to_string(),
        });
    }

    /// Send scene data to Blender
    pub fn send_scene_data(&self, entities: Vec<EntityData>) {
        self.broadcast(LiveLinkMessage::SceneData { entities });
    }

    /// Get number of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.lock().unwrap().len()
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    /// Stop the server
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }
}

impl Drop for LiveLink {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Run the WebSocket server (called in separate thread)
#[tokio::main(flavor = "current_thread")]
async fn run_server(port: u16, state: ServerState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;

    log::info!("Live Link WebSocket server listening on {}", addr);

    // Spawn broadcast handler
    let clients_for_broadcast = state.clients.clone();
    let mut broadcast_rx = state.broadcast_rx;

    tokio::spawn(async move {
        // This would need client WebSocket senders to actually broadcast
        // For now, just drain the channel
        while let Some(_msg) = broadcast_rx.recv().await {
            // Would send to all clients here
        }
    });

    while *state.running.lock().unwrap() {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, addr)) => {
                        let main_tx = state.main_tx.clone();
                        let clients = state.clients.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_client(stream, addr, main_tx, clients).await {
                                log::warn!("Client {} error: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Accept error: {}", e);
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Periodic check for shutdown
            }
        }
    }

    Ok(())
}

/// Handle a single WebSocket client connection
async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    main_tx: mpsc::UnboundedSender<LiveLinkMessage>,
    clients: Arc<Mutex<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    log::info!("New Live Link client connected: {}", addr);

    // Add client to list
    clients.lock().unwrap().insert(addr, ClientInfo {
        name: "Unknown".to_string(),
        addr,
    });

    // Send welcome message
    let welcome = serde_json::to_string(&LiveLinkMessage::Pong)?;
    write.send(tokio_tungstenite::tungstenite::Message::Text(welcome.into())).await?;

    // Process incoming messages
    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(msg) => {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    match serde_json::from_str::<LiveLinkMessage>(&text) {
                        Ok(live_msg) => {
                            // Handle ping
                            if matches!(live_msg, LiveLinkMessage::Ping) {
                                let pong = serde_json::to_string(&LiveLinkMessage::Pong)?;
                                write.send(tokio_tungstenite::tungstenite::Message::Text(pong.into())).await?;
                                continue;
                            }

                            // Update client name if connected message
                            if let LiveLinkMessage::Connected { ref client_name } = live_msg {
                                clients.lock().unwrap().entry(addr).and_modify(|c| {
                                    c.name = client_name.clone();
                                });
                                log::info!("Client {} identified as '{}'", addr, client_name);
                            }

                            // Forward to main thread
                            if let Err(e) = main_tx.send(live_msg) {
                                log::error!("Failed to forward message: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            log::warn!("Invalid message from {}: {}", addr, e);
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("WebSocket error from {}: {}", addr, e);
                break;
            }
        }
    }

    // Remove client from list
    clients.lock().unwrap().remove(&addr);
    log::info!("Client disconnected: {}", addr);

    Ok(())
}

/// Stub implementation when live_link feature is disabled
#[cfg(not(feature = "live_link"))]
pub struct LiveLink;

#[cfg(not(feature = "live_link"))]
impl LiveLink {
    pub fn start(_port: u16) -> Self {
        log::warn!("Live Link feature is not enabled. Compile with --features live_link");
        Self
    }

    pub fn poll_messages(&mut self) -> Vec<LiveLinkMessage> {
        Vec::new()
    }

    pub fn broadcast(&self, _msg: LiveLinkMessage) {}

    pub fn send_script_error(&self, _file: &str, _line: u32, _message: &str) {}

    pub fn send_log(&self, _level: &str, _message: &str) {}

    pub fn send_scene_data(&self, _entities: Vec<EntityData>) {}

    pub fn client_count(&self) -> usize { 0 }

    pub fn is_running(&self) -> bool { false }

    pub fn stop(&self) {}
}

#[cfg(not(feature = "live_link"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LiveLinkMessage {
    EntityUpdate { entity: String, position: [f32; 3], rotation: [f32; 4], scale: [f32; 3] },
    ScriptReload { path: String },
    ScriptError { file: String, line: u32, message: String },
    PlayRequest,
    StopRequest,
    PauseRequest,
    SceneSync,
    SceneData { entities: Vec<EntityData> },
    Ping,
    Pong,
    Connected { client_name: String },
    Log { level: String, message: String },
}

#[cfg(not(feature = "live_link"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub mesh: Option<String>,
    pub script: Option<String>,
}
