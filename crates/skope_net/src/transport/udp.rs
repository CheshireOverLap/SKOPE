//! UDP Transport — tokio background thread + mpsc channels (LiveLink pattern).
//!
//! Architecture:
//! ```text
//! Main Thread                          tokio Thread
//! ─────────────                      ────────────
//! UdpTransport {                     async fn transport_loop() {
//!   cmd_tx ──────────────────→         cmd_rx.recv()
//!   msg_rx ←──────────────────         msg_tx.send()
//!   event_rx ←────────────────         event_tx.send()
//! }                                    UdpSocket.send_to / recv_from
//!                                    }
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::mpsc;

use super::{Transport, PeerId, ConnectionEvent};
use super::packet::{PacketHeader, Channel, HEADER_SIZE};
use super::reliable::ReliabilityManager;

/// Commands sent from the main thread to the tokio thread.
enum TransportCmd {
    /// Send data to a specific peer address.
    SendTo(SocketAddr, Vec<u8>),
    /// Broadcast data to all known peers.
    Broadcast(Vec<u8>),
    /// Disconnect a peer.
    Disconnect(SocketAddr),
    /// Shutdown the transport thread.
    Shutdown,
}

/// Events from the tokio thread to the main thread.
enum TransportEvent {
    /// A message was received from a peer.
    Message(SocketAddr, Vec<u8>),
    /// A new peer connected (first message from unknown address).
    Connected(SocketAddr),
}

/// Configuration for UdpTransport.
pub struct UdpTransportConfig {
    /// Address to bind to. Server: "0.0.0.0:PORT", Client: "0.0.0.0:0".
    pub bind_addr: String,
    /// For client: the server address to connect to. None for server mode.
    pub server_addr: Option<String>,
}

/// UDP transport using a background tokio thread.
/// Implements the `Transport` trait for use with `NetworkTransport`.
pub struct UdpTransport {
    cmd_tx: mpsc::Sender<TransportCmd>,
    msg_rx: mpsc::Receiver<TransportEvent>,
    /// Peer address → assigned PeerId mapping.
    addr_to_peer: HashMap<SocketAddr, PeerId>,
    peer_to_addr: HashMap<PeerId, SocketAddr>,
    next_peer_id: PeerId,
    /// Pending connection events.
    pending_connections: Vec<ConnectionEvent>,
    /// Pending received messages.
    pending_messages: Vec<(PeerId, Vec<u8>)>,
    /// Reliability manager for ACK-based retransmission.
    reliability: ReliabilityManager,
    /// Whether the transport is server or client.
    is_server: bool,
    /// Bind error reported by the background thread (checked once).
    bind_error: Option<mpsc::Receiver<String>>,
}

impl UdpTransport {
    /// Start a new UDP transport.
    /// Spawns a background thread running a tokio runtime with a UDP socket.
    pub fn new(config: UdpTransportConfig) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let (err_tx, err_rx) = mpsc::channel();

        let bind_addr = config.bind_addr.clone();
        let server_addr = config.server_addr.clone();
        let is_server = server_addr.is_none();

        std::thread::spawn(move || {
            if let Err(e) = run_transport_loop(bind_addr, cmd_rx, event_tx) {
                let msg = format!("{}", e);
                log::error!("UDP transport error: {}", msg);
                let _ = err_tx.send(msg);
            }
        });

        let mut transport = Self {
            cmd_tx,
            msg_rx: event_rx,
            addr_to_peer: HashMap::new(),
            peer_to_addr: HashMap::new(),
            next_peer_id: if is_server { 1 } else { 0 },
            pending_connections: Vec::new(),
            pending_messages: Vec::new(),
            reliability: ReliabilityManager::default(),
            is_server,
            bind_error: Some(err_rx),
        };

        // Client: register server as peer 0
        if let Some(addr_str) = &server_addr {
            if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                transport.addr_to_peer.insert(addr, 0);
                transport.peer_to_addr.insert(0, addr);
                transport.pending_connections.push(ConnectionEvent::Connected(0));
            }
        }

        transport
    }

    /// Check for a bind error from the background thread.
    /// Returns Some(error_message) once if the socket failed to bind.
    pub fn take_bind_error(&mut self) -> Option<String> {
        if let Some(ref rx) = self.bind_error {
            match rx.try_recv() {
                Ok(err) => {
                    self.bind_error = None;
                    Some(err)
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.bind_error = None; // thread exited without error
                    None
                }
                Err(mpsc::TryRecvError::Empty) => None,
            }
        } else {
            None
        }
    }

    /// Drain events from the background thread and process them.
    fn drain_events(&mut self) {
        while let Ok(event) = self.msg_rx.try_recv() {
            match event {
                TransportEvent::Message(addr, data) => {
                    // Decode packet header
                    if data.len() < HEADER_SIZE {
                        continue;
                    }

                    let (header, payload) = match PacketHeader::decode(&data) {
                        Some(hp) => hp,
                        None => continue,
                    };

                    // Assign peer ID if new address
                    let peer_id = if let Some(&pid) = self.addr_to_peer.get(&addr) {
                        pid
                    } else if self.is_server {
                        let pid = self.next_peer_id;
                        self.next_peer_id += 1;
                        self.addr_to_peer.insert(addr, pid);
                        self.peer_to_addr.insert(pid, addr);
                        self.pending_connections.push(ConnectionEvent::Connected(pid));
                        pid
                    } else {
                        continue; // Client ignores unknown senders
                    };

                    match header.channel {
                        Channel::Unreliable => {
                            self.pending_messages.push((peer_id, payload.to_vec()));
                        }
                        Channel::Reliable => {
                            let channel = self.reliability.get_or_create(peer_id);
                            // Send ACK
                            let ack = super::reliable::ReliableChannel::make_ack(header.sequence);
                            if let Some(&peer_addr) = self.peer_to_addr.get(&peer_id) {
                                let _ = self.cmd_tx.send(TransportCmd::SendTo(peer_addr, ack));
                            }
                            // Deduplicate
                            if channel.accept_received(header.sequence) {
                                self.pending_messages.push((peer_id, payload.to_vec()));
                            }
                        }
                        Channel::Ack => {
                            let channel = self.reliability.get_or_create(peer_id);
                            channel.receive_ack(header.sequence);
                        }
                    }
                }
                TransportEvent::Connected(addr) => {
                    if !self.addr_to_peer.contains_key(&addr) && self.is_server {
                        let pid = self.next_peer_id;
                        self.next_peer_id += 1;
                        self.addr_to_peer.insert(addr, pid);
                        self.peer_to_addr.insert(pid, addr);
                        self.pending_connections.push(ConnectionEvent::Connected(pid));
                    }
                }
            }
        }
    }

    /// Send a raw packet (with header) to a peer address.
    fn raw_send(&self, addr: SocketAddr, data: Vec<u8>) {
        let _ = self.cmd_tx.send(TransportCmd::SendTo(addr, data));
    }

    /// Collect and send retransmissions for all peers.
    pub fn process_retransmits(&mut self) {
        let peer_ids: Vec<u32> = self.reliability.channels.keys().copied().collect();
        for peer_id in peer_ids {
            if let Some(addr) = self.peer_to_addr.get(&peer_id).copied() {
                if let Some(channel) = self.reliability.channels.get_mut(&peer_id) {
                    for packet in channel.collect_retransmits() {
                        self.raw_send(addr, packet);
                    }
                }
            }
        }
    }
}

impl Transport for UdpTransport {
    fn poll_messages(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        self.drain_events();
        self.process_retransmits();
        std::mem::take(&mut self.pending_messages)
    }

    fn send_to(&mut self, peer: PeerId, data: Vec<u8>) {
        let addr = match self.peer_to_addr.get(&peer) {
            Some(&a) => a,
            None => {
                log::warn!("UdpTransport::send_to: unknown peer {}", peer);
                return;
            }
        };

        let header = PacketHeader {
            channel: Channel::Unreliable,
            sequence: 0,
        };
        let packet = header.encode(&data);
        self.raw_send(addr, packet);
    }

    fn send_reliable(&mut self, peer: PeerId, data: Vec<u8>) {
        let addr = match self.peer_to_addr.get(&peer) {
            Some(&a) => a,
            None => {
                log::warn!("UdpTransport::send_reliable: unknown peer {}", peer);
                return;
            }
        };

        let channel = self.reliability.get_or_create(peer);
        let packet = channel.wrap_reliable(&data);
        self.raw_send(addr, packet);
    }

    fn broadcast(&mut self, data: Vec<u8>) {
        let header = PacketHeader {
            channel: Channel::Unreliable,
            sequence: 0,
        };
        let packet = header.encode(&data);
        let _ = self.cmd_tx.send(TransportCmd::Broadcast(packet));
    }

    fn broadcast_reliable(&mut self, data: Vec<u8>) {
        let peers: Vec<(PeerId, SocketAddr)> = self.peer_to_addr.iter()
            .map(|(&pid, &addr)| (pid, addr))
            .collect();
        for (peer_id, addr) in peers {
            let channel = self.reliability.get_or_create(peer_id);
            let packet = channel.wrap_reliable(&data);
            self.raw_send(addr, packet);
        }
    }

    fn poll_connections(&mut self) -> Vec<ConnectionEvent> {
        self.drain_events();
        std::mem::take(&mut self.pending_connections)
    }

    fn take_error(&mut self) -> Option<String> {
        self.take_bind_error()
    }

    fn disconnect(&mut self, peer: PeerId) {
        if let Some(addr) = self.peer_to_addr.remove(&peer) {
            self.addr_to_peer.remove(&addr);
            self.reliability.remove_peer(peer);
            let _ = self.cmd_tx.send(TransportCmd::Disconnect(addr));
            self.pending_connections.push(ConnectionEvent::Disconnected(peer));
        }
    }
}

impl Drop for UdpTransport {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(TransportCmd::Shutdown);
    }
}

/// Background tokio runtime loop for UDP socket I/O.
#[tokio::main(flavor = "current_thread")]
async fn run_transport_loop(
    bind_addr: String,
    cmd_rx: mpsc::Receiver<TransportCmd>,
    event_tx: mpsc::Sender<TransportEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let socket = tokio::net::UdpSocket::bind(&bind_addr).await?;
    log::info!("UDP transport bound to {}", socket.local_addr()?);

    let mut buf = vec![0u8; 65536];
    let mut known_peers: HashMap<SocketAddr, bool> = HashMap::new();

    loop {
        tokio::select! {
            // Receive from socket
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, addr)) => {
                        if !known_peers.contains_key(&addr) {
                            known_peers.insert(addr, true);
                            let _ = event_tx.send(TransportEvent::Connected(addr));
                        }
                        let data = buf[..len].to_vec();
                        let _ = event_tx.send(TransportEvent::Message(addr, data));
                    }
                    Err(e) => {
                        log::warn!("UDP recv error: {}", e);
                    }
                }
            }

            // Check for commands from main thread (non-blocking via sleep interval)
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(1)) => {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TransportCmd::SendTo(addr, data) => {
                            if let Err(e) = socket.send_to(&data, addr).await {
                                log::warn!("UDP send to {} error: {}", addr, e);
                            }
                        }
                        TransportCmd::Broadcast(data) => {
                            for &addr in known_peers.keys() {
                                if let Err(e) = socket.send_to(&data, addr).await {
                                    log::warn!("UDP broadcast to {} error: {}", addr, e);
                                }
                            }
                        }
                        TransportCmd::Disconnect(addr) => {
                            known_peers.remove(&addr);
                        }
                        TransportCmd::Shutdown => {
                            log::info!("UDP transport shutting down");
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: poll messages with retries, allowing time for the background thread.
    fn poll_with_retry(transport: &mut UdpTransport, max_ms: u64) -> Vec<(PeerId, Vec<u8>)> {
        let start = std::time::Instant::now();
        loop {
            let msgs = transport.poll_messages();
            if !msgs.is_empty() {
                return msgs;
            }
            if start.elapsed().as_millis() as u64 > max_ms {
                return Vec::new();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Helper: poll connections with retries.
    fn poll_connections_retry(transport: &mut UdpTransport, max_ms: u64) -> Vec<ConnectionEvent> {
        let start = std::time::Instant::now();
        loop {
            let events = transport.poll_connections();
            if !events.is_empty() {
                return events;
            }
            if start.elapsed().as_millis() as u64 > max_ms {
                return Vec::new();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn test_udp_localhost_roundtrip() {
        let port = 19876u16;
        let mut server = UdpTransport::new(UdpTransportConfig {
            bind_addr: format!("127.0.0.1:{}", port),
            server_addr: None,
        });

        // Wait for server to start
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Client connects to server
        let mut client = UdpTransport::new(UdpTransportConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            server_addr: Some(format!("127.0.0.1:{}", port)),
        });

        // Client should have an immediate Connected(0) for the server
        let client_events = poll_connections_retry(&mut client, 200);
        assert!(
            client_events.iter().any(|e| matches!(e, ConnectionEvent::Connected(0))),
            "Client should get Connected(0) for server"
        );

        // Client sends a message to server (peer 0)
        client.send_to(0, b"hello server".to_vec());

        // Server should receive the message
        let server_msgs = poll_with_retry(&mut server, 500);
        assert!(!server_msgs.is_empty(), "Server should receive a message");
        let (peer_id, data) = &server_msgs[0];
        assert_eq!(data, b"hello server");

        // peer_id should be 1 (first client)
        assert_eq!(*peer_id, 1, "Client should be assigned peer_id 1");

        // Server sends a reply to client (peer 1)
        server.send_to(1, b"hello client".to_vec());

        // Client should receive the reply
        let client_msgs = poll_with_retry(&mut client, 500);
        assert!(!client_msgs.is_empty(), "Client should receive a reply");
        let (server_peer, reply_data) = &client_msgs[0];
        assert_eq!(*server_peer, 0, "Reply should come from peer 0 (server)");
        assert_eq!(reply_data, b"hello client");
    }

    #[test]
    fn test_udp_disconnect() {
        let port = 19877u16;
        let mut server = UdpTransport::new(UdpTransportConfig {
            bind_addr: format!("127.0.0.1:{}", port),
            server_addr: None,
        });
        std::thread::sleep(std::time::Duration::from_millis(50));

        let mut client = UdpTransport::new(UdpTransportConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            server_addr: Some(format!("127.0.0.1:{}", port)),
        });

        // Client sends a message so server discovers it
        client.send_to(0, b"hello".to_vec());
        let _msgs = poll_with_retry(&mut server, 500);

        // Server disconnects the client
        server.disconnect(1);

        let events = server.poll_connections();
        assert!(
            events.iter().any(|e| matches!(e, ConnectionEvent::Disconnected(1))),
            "Should get Disconnected event for peer 1"
        );
    }
}
