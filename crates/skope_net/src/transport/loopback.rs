//! Loopback transport for local testing.
//!
//! Provides `LoopbackEndpoint` and `loopback_pair()` for unit tests.

use std::collections::VecDeque;

use super::{Transport, PeerId, ConnectionEvent};

/// Loopback transport endpoint for local testing.
/// Use `loopback_pair()` to create a server/client endpoint pair.
pub struct LoopbackEndpoint {
    /// The peer ID that the remote side appears as.
    peer_id: PeerId,
    /// Messages received from the remote side.
    inbox: VecDeque<(PeerId, Vec<u8>)>,
    /// Messages queued for sending to the remote side.
    outbox: Vec<Vec<u8>>,
    /// Whether we are "connected" to the remote side.
    connected: bool,
    /// Pending connection events.
    connection_events: VecDeque<ConnectionEvent>,
}

/// Create a server(peer_id=0) and client(peer_id=1) endpoint pair.
/// Call `flush_loopback(&mut server, &mut client)` each tick to exchange messages.
pub fn loopback_pair() -> (LoopbackEndpoint, LoopbackEndpoint) {
    let server = LoopbackEndpoint {
        peer_id: 1, // server sees client as peer 1
        inbox: VecDeque::new(),
        outbox: Vec::new(),
        connected: true,
        connection_events: VecDeque::from([ConnectionEvent::Connected(1)]),
    };
    let client = LoopbackEndpoint {
        peer_id: 0, // client sees server as peer 0
        inbox: VecDeque::new(),
        outbox: Vec::new(),
        connected: true,
        connection_events: VecDeque::from([ConnectionEvent::Connected(0)]),
    };
    (server, client)
}

/// Flush messages bidirectionally between two loopback endpoints.
/// Call once per tick in tests.
pub fn flush_loopback(a: &mut LoopbackEndpoint, b: &mut LoopbackEndpoint) {
    // a → b: b receives messages tagged with b.peer_id (how b sees a)
    for data in a.outbox.drain(..) {
        b.inbox.push_back((b.peer_id, data));
    }
    // b → a: a receives messages tagged with a.peer_id (how a sees b)
    for data in b.outbox.drain(..) {
        a.inbox.push_back((a.peer_id, data));
    }
}

impl Transport for LoopbackEndpoint {
    fn poll_messages(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        self.inbox.drain(..).collect()
    }

    fn send_to(&mut self, _peer: PeerId, data: Vec<u8>) {
        if self.connected {
            self.outbox.push(data);
        }
    }

    fn broadcast(&mut self, data: Vec<u8>) {
        if self.connected {
            self.outbox.push(data);
        }
    }

    fn poll_connections(&mut self) -> Vec<ConnectionEvent> {
        self.connection_events.drain(..).collect()
    }

    fn disconnect(&mut self, _peer: PeerId) {
        self.connected = false;
        self.connection_events
            .push_back(ConnectionEvent::Disconnected(self.peer_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loopback_transport() {
        let (mut server, mut client) = loopback_pair();

        server.send_to(1, b"hello client".to_vec());
        client.send_to(0, b"hello server".to_vec());

        assert!(server.poll_messages().is_empty());
        assert!(client.poll_messages().is_empty());

        flush_loopback(&mut server, &mut client);

        let server_msgs = server.poll_messages();
        assert_eq!(server_msgs.len(), 1);
        assert_eq!(server_msgs[0].0, 1);
        assert_eq!(server_msgs[0].1, b"hello server");

        let client_msgs = client.poll_messages();
        assert_eq!(client_msgs.len(), 1);
        assert_eq!(client_msgs[0].0, 0);
        assert_eq!(client_msgs[0].1, b"hello client");
    }

    #[test]
    fn test_loopback_connection_events() {
        let (mut server, mut client) = loopback_pair();

        let server_events = server.poll_connections();
        assert_eq!(server_events.len(), 1);
        assert!(matches!(server_events[0], ConnectionEvent::Connected(1)));

        let client_events = client.poll_connections();
        assert_eq!(client_events.len(), 1);
        assert!(matches!(client_events[0], ConnectionEvent::Connected(0)));

        assert!(server.poll_connections().is_empty());
    }

    #[test]
    fn test_loopback_broadcast() {
        let (mut server, mut client) = loopback_pair();

        server.broadcast(b"broadcast msg".to_vec());
        flush_loopback(&mut server, &mut client);

        let msgs = client.poll_messages();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].1, b"broadcast msg");
    }
}
