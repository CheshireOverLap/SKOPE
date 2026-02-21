//! ACK-based reliability layer for UDP transport.
//!
//! Provides sequence tracking, retransmission of unacknowledged reliable messages,
//! and deduplication of received messages.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use super::packet::{PacketHeader, Channel};

/// A pending reliable message awaiting ACK.
struct PendingMessage {
    sequence: u16,
    payload: Vec<u8>,
    send_time: Instant,
    retransmit_count: u32,
}

/// Per-peer reliability state.
pub struct ReliableChannel {
    /// Next outgoing sequence number.
    next_send_seq: u16,
    /// Messages sent but not yet acknowledged.
    pending_acks: VecDeque<PendingMessage>,
    /// Set of received sequence numbers for deduplication.
    received_seqs: HashSet<u16>,
    /// Sliding window to limit memory usage of received_seqs.
    /// Sequences below this are considered implicitly received.
    received_floor: u16,
    /// Retransmission timeout.
    pub retransmit_interval: Duration,
    /// Maximum retransmission attempts before giving up.
    pub max_retransmits: u32,
}

impl Default for ReliableChannel {
    fn default() -> Self {
        Self {
            next_send_seq: 0,
            pending_acks: VecDeque::new(),
            received_seqs: HashSet::new(),
            received_floor: 0,
            retransmit_interval: Duration::from_millis(200),
            max_retransmits: 10,
        }
    }
}

impl ReliableChannel {
    /// Wrap a payload for reliable delivery. Returns the encoded packet (header + payload).
    pub fn wrap_reliable(&mut self, payload: &[u8]) -> Vec<u8> {
        let seq = self.next_send_seq;
        self.next_send_seq = self.next_send_seq.wrapping_add(1);

        let header = PacketHeader {
            channel: Channel::Reliable,
            sequence: seq,
        };

        self.pending_acks.push_back(PendingMessage {
            sequence: seq,
            payload: payload.to_vec(),
            send_time: Instant::now(),
            retransmit_count: 0,
        });

        header.encode(payload)
    }

    /// Create an ACK packet for a received reliable sequence.
    pub fn make_ack(sequence: u16) -> Vec<u8> {
        let header = PacketHeader {
            channel: Channel::Ack,
            sequence,
        };
        header.encode(&[])
    }

    /// Process a received ACK — removes the pending message from the retransmit queue.
    pub fn receive_ack(&mut self, sequence: u16) {
        self.pending_acks.retain(|m| m.sequence != sequence);
    }

    /// Check if a received reliable sequence is a duplicate.
    /// Returns `true` if this is a NEW (non-duplicate) message.
    pub fn accept_received(&mut self, sequence: u16) -> bool {
        // If below the floor, it's already been processed
        if self.wrapping_lt(sequence, self.received_floor) {
            return false;
        }
        let is_new = self.received_seqs.insert(sequence);

        // Advance floor if we can
        while self.received_seqs.contains(&self.received_floor) {
            self.received_seqs.remove(&self.received_floor);
            self.received_floor = self.received_floor.wrapping_add(1);
        }

        is_new
    }

    /// Collect packets that need retransmission (timed out, not yet ACKed).
    /// Returns list of encoded packets ready to send.
    pub fn collect_retransmits(&mut self) -> Vec<Vec<u8>> {
        let now = Instant::now();
        let mut retransmits = Vec::new();
        let mut expired_indices = Vec::new();

        for (i, pending) in self.pending_acks.iter_mut().enumerate() {
            if now.duration_since(pending.send_time) >= self.retransmit_interval {
                if pending.retransmit_count >= self.max_retransmits {
                    expired_indices.push(i);
                    continue;
                }
                pending.retransmit_count += 1;
                pending.send_time = now;

                let header = PacketHeader {
                    channel: Channel::Reliable,
                    sequence: pending.sequence,
                };
                retransmits.push(header.encode(&pending.payload));
            }
        }

        // Remove expired messages (iterate in reverse to preserve indices)
        for &i in expired_indices.iter().rev() {
            let msg = self.pending_acks.remove(i);
            if let Some(msg) = msg {
                log::warn!(
                    "Reliable message seq={} dropped after {} retransmits",
                    msg.sequence,
                    msg.retransmit_count
                );
            }
        }

        retransmits
    }

    /// Number of messages awaiting ACK.
    pub fn pending_count(&self) -> usize {
        self.pending_acks.len()
    }

    /// Wrapping less-than comparison for u16 sequence numbers.
    fn wrapping_lt(&self, a: u16, b: u16) -> bool {
        let diff = a.wrapping_sub(b);
        diff > 32768
    }
}

/// Per-peer reliability state tracker.
#[derive(Default)]
pub struct ReliabilityManager {
    pub channels: HashMap<u32, ReliableChannel>,
}

impl ReliabilityManager {
    pub fn get_or_create(&mut self, peer_id: u32) -> &mut ReliableChannel {
        self.channels.entry(peer_id).or_default()
    }

    pub fn remove_peer(&mut self, peer_id: u32) {
        self.channels.remove(&peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reliable_ack_retransmit() {
        let mut channel = ReliableChannel::default();
        channel.retransmit_interval = Duration::from_millis(0); // immediate retransmit for testing

        // Send a reliable message
        let packet = channel.wrap_reliable(b"important data");
        assert_eq!(channel.pending_count(), 1);

        // Before ACK, retransmit should produce one packet
        let retransmits = channel.collect_retransmits();
        assert_eq!(retransmits.len(), 1);

        // ACK it
        channel.receive_ack(0);
        assert_eq!(channel.pending_count(), 0);

        // No more retransmits
        let retransmits = channel.collect_retransmits();
        assert!(retransmits.is_empty());

        // Verify the original packet is valid
        let (header, payload) = PacketHeader::decode(&packet).unwrap();
        assert_eq!(header.channel, Channel::Reliable);
        assert_eq!(header.sequence, 0);
        assert_eq!(payload, b"important data");
    }

    #[test]
    fn test_reliable_deduplication() {
        let mut channel = ReliableChannel::default();

        // First receive → accepted
        assert!(channel.accept_received(0));
        // Duplicate → rejected
        assert!(!channel.accept_received(0));

        // Next sequence → accepted
        assert!(channel.accept_received(1));
        assert!(!channel.accept_received(1));
    }

    #[test]
    fn test_reliable_floor_advance() {
        let mut channel = ReliableChannel::default();

        // Receive out of order: 0, 2, 1
        assert!(channel.accept_received(0));
        assert!(channel.accept_received(2));
        assert!(channel.accept_received(1));

        // Floor should have advanced past all three
        assert_eq!(channel.received_floor, 3);

        // 0, 1, 2 are now below floor
        assert!(!channel.accept_received(0));
        assert!(!channel.accept_received(1));
        assert!(!channel.accept_received(2));

        // 3 is new
        assert!(channel.accept_received(3));
    }

    #[test]
    fn test_sequence_wrapping() {
        let mut channel = ReliableChannel {
            next_send_seq: u16::MAX,
            ..Default::default()
        };

        let _packet1 = channel.wrap_reliable(b"a");
        assert_eq!(channel.next_send_seq, 0); // wrapped

        let _packet2 = channel.wrap_reliable(b"b");
        assert_eq!(channel.next_send_seq, 1);
    }
}
