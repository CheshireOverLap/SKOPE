use skope_ecs::prelude::*;
use std::collections::{HashMap, HashSet};
use bitvec::prelude::*;
use glam;

use crate::transport::Transport;

/// 네트워크 전체 상태 (LiveLink 패턴 참조: mpsc 채널 + 별도 스레드)
#[derive(Resource)]
pub struct NetworkState {
    /// 현재 역할
    pub role: SessionRole,
    /// 다음 NetworkId 할당값 (서버만 증가)
    pub next_network_id: u64,
    /// NetworkId → Entity 매핑 (양방향 조회)
    pub net_to_entity: HashMap<u64, Entity>,
    pub entity_to_net: HashMap<Entity, u64>,
    /// 연결된 피어 수
    pub peer_count: u32,
    /// 연결된 피어 ID 셋 (Handshake 중복 방지)
    pub connected_peers: HashSet<u32>,
    /// 현재 틱 번호
    pub tick: u64,
    /// 클라이언트 전용: 서버가 할당한 로컬 피어 ID
    pub local_peer_id: u32,
    /// 서버 전용: 피어 ID → 소유 플레이어 엔티티 매핑
    pub peer_to_entity: HashMap<u32, Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    /// 로컬 서버 (솔로 = peer_count 0, 멀티 = peer_count 1+). 기본값.
    Server,
    /// 원격 서버에 접속한 클라이언트.
    Client,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            role: SessionRole::Server,
            next_network_id: 1,
            net_to_entity: HashMap::new(),
            entity_to_net: HashMap::new(),
            peer_count: 0,
            connected_peers: HashSet::new(),
            tick: 0,
            local_peer_id: 0,
            peer_to_entity: HashMap::new(),
        }
    }
}

impl NetworkState {
    /// NetworkId 할당 (서버 전용)
    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_network_id;
        self.next_network_id += 1;
        id
    }

    /// 엔티티-NetworkId 매핑 등록
    pub fn register(&mut self, entity: Entity, net_id: u64) {
        self.net_to_entity.insert(net_id, entity);
        self.entity_to_net.insert(entity, net_id);
    }

}

/// Dirty 추적 리소스 — BitVec 최적화.
/// net_id - 1 인덱스 (순차 할당이므로 dense).
#[derive(Resource)]
pub struct DirtyTracker {
    /// 이번 프레임에 변경된 비트 (net_id - 1 인덱스)
    dirty: BitVec,
    /// 누적 dirty (아직 전송 안 된 것)
    accumulated: BitVec,
}

impl Default for DirtyTracker {
    fn default() -> Self {
        Self {
            dirty: BitVec::new(),
            accumulated: BitVec::new(),
        }
    }
}

impl DirtyTracker {
    /// Ensure the bitvecs are large enough for the given net_id.
    fn ensure_capacity(&mut self, net_id: u64) {
        let idx = net_id.saturating_sub(1) as usize;
        if idx >= self.dirty.len() {
            self.dirty.resize(idx + 1, false);
            self.accumulated.resize(idx + 1, false);
        }
    }

    pub fn mark_dirty(&mut self, net_id: u64) {
        self.ensure_capacity(net_id);
        let idx = net_id.saturating_sub(1) as usize;
        self.dirty.set(idx, true);
        self.accumulated.set(idx, true);
    }

    pub fn drain_accumulated(&mut self) -> Vec<u64> {
        let result: Vec<u64> = self.accumulated.iter_ones()
            .map(|idx| idx as u64 + 1)
            .collect();
        self.accumulated.fill(false);
        result
    }

    pub fn clear_frame(&mut self) {
        self.dirty.fill(false);
    }

    /// Check if a specific net_id is dirty this frame.
    pub fn is_dirty(&self, net_id: u64) -> bool {
        let idx = net_id.saturating_sub(1) as usize;
        if idx < self.dirty.len() {
            self.dirty[idx]
        } else {
            false
        }
    }

    /// Get the set of dirty net_ids this frame (without draining).
    pub fn dirty_this_frame(&self) -> HashSet<u64> {
        self.dirty.iter_ones()
            .map(|idx| idx as u64 + 1)
            .collect()
    }
}

/// 네트워크 설정
#[derive(Resource)]
pub struct NetworkConfig {
    /// 서버 틱 레이트 (Hz)
    pub tick_rate: u32,
    /// 최대 연결 수
    pub max_connections: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self { tick_rate: 30, max_connections: 32 }
    }
}

/// Transport wrapper (NonSend resource — real sockets may not be Send).
pub struct NetworkTransport {
    inner: Option<Box<dyn Transport>>,
}

impl Default for NetworkTransport {
    fn default() -> Self {
        Self { inner: None }
    }
}

impl NetworkTransport {
    /// Create a transport resource wrapping an actual transport implementation.
    pub fn new(transport: Box<dyn Transport>) -> Self {
        Self { inner: Some(transport) }
    }

    /// Get mutable access to the inner transport.
    pub fn as_mut(&mut self) -> Option<&mut dyn Transport> {
        self.inner.as_mut().map(|b| &mut **b as &mut dyn Transport)
    }

}

/// Previous-tick snapshot cache for delta computation.
/// Maps net_id → Vec<(component_name, serialized_bytes)>.
#[derive(Resource, Default)]
pub struct SnapshotCache {
    pub cache: HashMap<u64, Vec<(String, Vec<u8>)>>,
}

/// Game-level position extractor callback.
/// Register this resource so the network layer can read entity positions
/// without depending on game-specific Transform types.
#[derive(Resource)]
pub struct PositionExtractor {
    pub extract: Box<dyn Fn(&World, Entity) -> Option<glam::Vec3> + Send + Sync>,
}

impl PositionExtractor {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&World, Entity) -> Option<glam::Vec3> + Send + Sync + 'static,
    {
        Self { extract: Box::new(f) }
    }
}

/// Game-level Transform accessor for network interpolation.
/// Provides both extract (read position+rotation) and apply (write) callbacks.
/// This allows skope_net to read/write game-specific Transform components
/// without depending on skope_core.
#[derive(Resource)]
pub struct TransformAccessor {
    pub extract: Box<dyn Fn(&World, Entity) -> Option<(glam::Vec3, glam::Quat)> + Send + Sync>,
    pub apply: Box<dyn Fn(&mut World, Entity, glam::Vec3, glam::Quat) + Send + Sync>,
}

impl TransformAccessor {
    pub fn new<E, A>(extract: E, apply: A) -> Self
    where
        E: Fn(&World, Entity) -> Option<(glam::Vec3, glam::Quat)> + Send + Sync + 'static,
        A: Fn(&mut World, Entity, glam::Vec3, glam::Quat) + Send + Sync + 'static,
    {
        Self {
            extract: Box::new(extract),
            apply: Box::new(apply),
        }
    }
}

/// Frame delta time — set by the game loop each frame.
/// Network systems use this for frame-rate-independent interpolation and prediction blending.
#[derive(Resource)]
pub struct FrameTime {
    pub delta_seconds: f32,
}

impl Default for FrameTime {
    fn default() -> Self {
        Self { delta_seconds: 1.0 / 60.0 }
    }
}

/// Per-peer connection metadata (server-side).
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub peer_id: u32,
    /// Round-trip time estimate in milliseconds.
    pub rtt_ms: f32,
    /// Tick when last message was received from this peer.
    pub last_heard_tick: u64,
    /// Client display name (from Handshake).
    pub client_name: String,
}

/// Connection tracking resource (server-side).
#[derive(Resource, Default)]
pub struct ConnectionTracker {
    pub connections: HashMap<u32, ConnectionInfo>,
    /// Ticks of inactivity before a peer is timed out. Default: 150 (5 sec at 30Hz).
    pub timeout_ticks: u64,
    /// Ticks between heartbeat/keepalive sends. Default: 30 (1 sec at 30Hz).
    pub heartbeat_interval_ticks: u64,
    /// Last tick a heartbeat was sent.
    pub last_heartbeat_tick: u64,
}

impl ConnectionTracker {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            timeout_ticks: 150,
            heartbeat_interval_ticks: 30,
            last_heartbeat_tick: 0,
        }
    }

    /// Update last_heard for a peer.
    pub fn touch(&mut self, peer_id: u32, tick: u64) {
        if let Some(info) = self.connections.get_mut(&peer_id) {
            info.last_heard_tick = tick;
        }
    }

    /// Add a new connection.
    pub fn add_peer(&mut self, peer_id: u32, name: String, tick: u64) {
        self.connections.insert(peer_id, ConnectionInfo {
            peer_id,
            rtt_ms: 0.0,
            last_heard_tick: tick,
            client_name: name,
        });
    }

    /// Remove a peer.
    pub fn remove_peer(&mut self, peer_id: u32) {
        self.connections.remove(&peer_id);
    }

    /// Find peers that have timed out.
    pub fn timed_out_peers(&self, current_tick: u64) -> Vec<u32> {
        self.connections
            .iter()
            .filter(|(_, info)| current_tick.saturating_sub(info.last_heard_tick) > self.timeout_ticks)
            .map(|(&pid, _)| pid)
            .collect()
    }
}
