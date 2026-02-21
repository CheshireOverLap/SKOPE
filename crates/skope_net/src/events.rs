use skope_ecs::prelude::*;

#[derive(Event, Debug, Clone)]
pub enum NetworkEvent {
    /// 피어 연결됨
    PeerConnected { peer_id: u32 },
    /// 피어 연결 해제
    PeerDisconnected { peer_id: u32 },
    /// 엔티티 디스폰
    EntityDespawned { net_id: u64 },
}
