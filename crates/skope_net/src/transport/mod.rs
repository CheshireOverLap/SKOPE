//! Transport abstraction for network communication.
//!
//! Provides the `Transport` trait and multiple implementations:
//! - `LoopbackEndpoint` — local testing (loopback.rs)
//! - `UdpTransport` — real UDP sockets via tokio (udp.rs)
//!
//! Packet framing and reliability are provided by sub-modules:
//! - `packet.rs` — channel + sequence header
//! - `reliable.rs` — ACK-based retransmission

pub mod loopback;
pub mod packet;
pub mod reliable;
pub mod udp;

pub type PeerId = u32;

#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    Connected(PeerId),
    Disconnected(PeerId),
}

/// Transport abstraction. Real sockets (UDP/WebSocket) implement this trait.
pub trait Transport: 'static {
    /// Drain pending received messages (non-blocking).
    fn poll_messages(&mut self) -> Vec<(PeerId, Vec<u8>)>;
    /// Send data to a specific peer (unreliable — may be lost).
    fn send_to(&mut self, peer: PeerId, data: Vec<u8>);
    /// Send data to a specific peer with guaranteed delivery (ACK + retransmit).
    /// Use for critical messages: Handshake, HandshakeResponse, Snapshot, Despawn, RPC.
    fn send_reliable(&mut self, peer: PeerId, data: Vec<u8>) {
        // Default: fall back to unreliable (loopback / test transports)
        self.send_to(peer, data);
    }
    /// Broadcast data to all connected peers (unreliable).
    fn broadcast(&mut self, data: Vec<u8>);
    /// Broadcast data to all connected peers with guaranteed delivery.
    fn broadcast_reliable(&mut self, data: Vec<u8>) {
        // Default: fall back to unreliable broadcast
        self.broadcast(data);
    }
    /// Drain connection/disconnection events.
    fn poll_connections(&mut self) -> Vec<ConnectionEvent>;
    /// Disconnect a specific peer.
    fn disconnect(&mut self, peer: PeerId);
    /// Check for a startup/bind error. Returns Some(msg) once if an error occurred.
    fn take_error(&mut self) -> Option<String> { None }
}

// Re-exports for backward compatibility
pub use loopback::{LoopbackEndpoint, loopback_pair, flush_loopback};
pub use udp::{UdpTransport, UdpTransportConfig};
