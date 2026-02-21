use skope_ecs::prelude::*;
use serde::{Serialize, Deserialize};

/// 네트워크 오브젝트 ID (서버가 할당, 모든 피어에서 동일)
/// Iris의 FNetRefHandle 대응 (64비트 → u64 카운터로 단순화)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NetworkId(pub u64);

/// 네트워크 역할 (UE의 ENetRole 대응)
/// 솔로 플레이도 로컬 서버로 동작하므로 기본값은 Authority.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetRole {
    /// 이 엔티티의 권위자 (서버/솔로). 상태를 복제함. 기본값.
    Authority,
    /// 서버로부터 상태를 받아 적용하는 원격 엔티티.
    SimulatedProxy,
    /// 소유 클라이언트: 입력 전송 + 로컬 예측.
    AutonomousProxy,
}

impl Default for NetRole {
    fn default() -> Self { Self::Authority }
}

/// 복제 대상 마커. 이 컴포넌트가 있는 엔티티만 네트워크 동기화.
/// Iris의 ReplicationBridge::StartReplicating 대응.
#[derive(Component, Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Replicated;

/// 이 엔티티를 소유한 연결 ID.
/// PlayerController 등에 부착. Iris의 NetConnection owner 대응.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetOwner(pub u32);
