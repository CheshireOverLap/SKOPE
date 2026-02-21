pub mod components;
pub mod resources;
pub mod events;
pub mod protocol;
pub mod systems;
pub mod transport;
pub mod registry;
pub mod input;
pub mod rpc;
pub mod prediction;
pub mod interpolation;
pub mod tick;
pub mod relevancy;
pub mod priority;
pub mod quantize;
pub mod dormancy;

pub mod prelude {
    pub use crate::components::*;
    pub use crate::resources::*;
    pub use crate::events::*;
    pub use crate::systems::*;
    pub use crate::transport::{Transport, PeerId, ConnectionEvent, UdpTransport, UdpTransportConfig};
    pub use crate::registry::NetComponentRegistry;
    pub use crate::input::{InputPayload, InputBuffer};
    pub use crate::rpc::RpcRegistry;
    pub use crate::resources::ConnectionTracker;
    pub use crate::prediction::PredictionBuffer;
    pub use crate::interpolation::NetInterpolation;
    pub use crate::tick::TickSync;
    pub use crate::relevancy::SpatialGrid;
    pub use crate::priority::{ReplicationPriority, BandwidthBudget};
    pub use crate::quantize::{QVec3, QQuat, QuantizedTransform};
    pub use crate::dormancy::DormancyTracker;
}

pub use prelude::*;
