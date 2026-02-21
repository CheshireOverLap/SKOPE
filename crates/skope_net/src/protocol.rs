use serde::{Serialize, Deserialize};

/// 네트워크 메시지 (Phase 1: 타입 정의만. 실제 전송은 Phase 2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetMessage {
    /// 핸드셰이크
    Handshake { client_name: String },
    HandshakeResponse { peer_id: u32, tick: u64 },

    /// 엔티티 스냅샷 (전체 상태)
    /// ComponentRegistry의 extract_entity() 결과를 그대로 담음
    Snapshot {
        tick: u64,
        entities: Vec<EntitySnapshot>,
    },

    /// 엔티티 상태 델타 (변경분만)
    Delta {
        tick: u64,
        entities: Vec<EntityDelta>,
    },

    /// RPC 호출 (Iris NetRPC 대응, 단순화)
    Rpc {
        net_id: u64,
        method: String,
        args: Vec<u8>,
    },

    /// 엔티티 디스폰 (서버 → 클라이언트)
    Despawn {
        tick: u64,
        net_ids: Vec<u64>,
    },

    /// 입력 (클라이언트 → 서버)
    Input {
        tick: u64,
        data: Vec<u8>,
    },

    /// Ping — client sends to server for RTT measurement.
    Ping {
        local_tick: u64,
        send_time_ms: u64,
    },

    /// Pong — server responds with its tick and the client's original data.
    Pong {
        remote_tick: u64,
        local_tick: u64,
        send_time_ms: u64,
    },
}

impl NetMessage {
    /// Serialize this message to bytes (bincode).
    pub fn to_bytes(&self) -> Option<Vec<u8>> {
        bincode::serialize(self).ok()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub net_id: u64,
    /// 컴포넌트 이름 → 직렬화된 데이터 (ComponentRegistry 호환)
    pub components: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDelta {
    pub net_id: u64,
    /// 변경된 컴포넌트만
    pub changed_components: Vec<(String, Vec<u8>)>,
}
