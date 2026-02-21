//! Packet framing — channel + sequence header for UDP transport.
//!
//! Each UDP packet starts with a 4-byte header:
//! - byte 0: channel (Unreliable=0, Reliable=1, Ack=2)
//! - byte 1: reserved (0)
//! - bytes 2-3: sequence number (little-endian u16)

use serde::{Serialize, Deserialize};

/// Logical channel for packet delivery guarantees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Channel {
    /// Fire-and-forget. Used for Delta, Input (frequent, loss-tolerant).
    Unreliable = 0,
    /// Guaranteed delivery with ordering. Used for Handshake, Despawn, RPC.
    Reliable = 1,
    /// ACK response for reliable messages.
    Ack = 2,
}

impl Channel {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Channel::Unreliable),
            1 => Some(Channel::Reliable),
            2 => Some(Channel::Ack),
            _ => None,
        }
    }
}

/// 4-byte packet header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub channel: Channel,
    pub sequence: u16,
}

pub const HEADER_SIZE: usize = 4;

impl PacketHeader {
    /// Write header to a 4-byte buffer.
    pub fn write(&self, buf: &mut [u8; HEADER_SIZE]) {
        buf[0] = self.channel as u8;
        buf[1] = 0; // reserved
        buf[2] = (self.sequence & 0xFF) as u8;
        buf[3] = (self.sequence >> 8) as u8;
    }

    /// Read header from a 4-byte buffer.
    pub fn read(buf: &[u8; HEADER_SIZE]) -> Option<Self> {
        let channel = Channel::from_u8(buf[0])?;
        let sequence = u16::from_le_bytes([buf[2], buf[3]]);
        Some(Self { channel, sequence })
    }

    /// Encode header + payload into a single buffer.
    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
        let mut header_bytes = [0u8; HEADER_SIZE];
        self.write(&mut header_bytes);
        buf.extend_from_slice(&header_bytes);
        buf.extend_from_slice(payload);
        buf
    }

    /// Decode header and return (header, payload_slice).
    pub fn decode(data: &[u8]) -> Option<(Self, &[u8])> {
        if data.len() < HEADER_SIZE {
            return None;
        }
        let header_bytes: [u8; HEADER_SIZE] = [data[0], data[1], data[2], data[3]];
        let header = Self::read(&header_bytes)?;
        Some((header, &data[HEADER_SIZE..]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_roundtrip() {
        let header = PacketHeader {
            channel: Channel::Reliable,
            sequence: 12345,
        };
        let mut buf = [0u8; HEADER_SIZE];
        header.write(&mut buf);
        let decoded = PacketHeader::read(&buf).unwrap();
        assert_eq!(decoded.channel, Channel::Reliable);
        assert_eq!(decoded.sequence, 12345);
    }

    #[test]
    fn test_encode_decode() {
        let header = PacketHeader {
            channel: Channel::Unreliable,
            sequence: 42,
        };
        let payload = b"hello world";
        let encoded = header.encode(payload);

        let (decoded_header, decoded_payload) = PacketHeader::decode(&encoded).unwrap();
        assert_eq!(decoded_header.channel, Channel::Unreliable);
        assert_eq!(decoded_header.sequence, 42);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_decode_too_short() {
        assert!(PacketHeader::decode(&[0, 1, 2]).is_none());
    }

    #[test]
    fn test_channel_from_u8() {
        assert_eq!(Channel::from_u8(0), Some(Channel::Unreliable));
        assert_eq!(Channel::from_u8(1), Some(Channel::Reliable));
        assert_eq!(Channel::from_u8(2), Some(Channel::Ack));
        assert_eq!(Channel::from_u8(3), None);
    }
}
