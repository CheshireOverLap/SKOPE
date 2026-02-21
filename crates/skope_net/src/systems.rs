//! Network replication systems.
//!
//! - `network_id_assignment_system`: auto-assign NetworkId to Replicated entities (server only)
//! - `network_send_system`: serialize dirty entities → per-peer send_to()
//! - `network_receive_system`: transport.poll_messages() → apply to World
//! - `network_tick_system`: increment tick counter

use std::collections::HashSet;
use skope_ecs::prelude::*;
use crate::components::{NetworkId, NetRole, NetOwner, Replicated};
use crate::events::NetworkEvent;
use crate::input::{InputPayload, InputBuffer};
use crate::protocol::{NetMessage, EntitySnapshot, EntityDelta};
use crate::resources::{
    NetworkState, NetworkTransport, SnapshotCache, DirtyTracker, SessionRole,
    ConnectionTracker, PositionExtractor, TransformAccessor, FrameTime,
};
use crate::registry::NetComponentRegistry;
use crate::rpc::RpcRegistry;
use crate::prediction::{PredictionBuffer, PredictedState};
use crate::interpolation::NetInterpolation;
use crate::tick::TickSync;
use crate::relevancy::SpatialGrid;

/// Assign NetworkId to Replicated entities that don't have one yet (server only).
pub fn network_id_assignment_system(world: &mut World) {
    let is_server = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Server)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    // Collect entities needing ID assignment
    let entities_needing_id: Vec<Entity> = {
        let all = world.alive_entities();
        all.into_iter()
            .filter(|&e| {
                world.has_component::<Replicated>(e) && !world.has_component::<NetworkId>(e)
            })
            .collect()
    };

    if entities_needing_id.is_empty() {
        return;
    }

    let mut net_state = world
        .remove_resource::<NetworkState>()
        .expect("NetworkState missing");

    for entity in entities_needing_id {
        let net_id = net_state.allocate_id();
        net_state.register(entity, net_id);
        world
            .entity_mut(entity)
            .insert(NetworkId(net_id))
            .insert(NetRole::Authority);
        log::trace!("Assigned NetworkId({}) to {:?}", net_id, entity);
    }

    world.insert_resource(net_state);
}

/// Serialize Replicated entities and send per-peer deltas via transport (server only).
/// Uses Phase 6 infrastructure: SpatialGrid (relevancy), DormancyTracker,
/// ReplicationPriority (distance-based), BandwidthBudget (per-peer cap).
/// Solo mode (peer_count == 0): clears frame and returns immediately — zero overhead.
pub fn network_send_system(world: &mut World) {
    let peer_count = world
        .get_resource::<NetworkState>()
        .map(|s| s.peer_count)
        .unwrap_or(0);

    if let Some(dirty) = world.get_resource_mut::<DirtyTracker>() {
        dirty.clear_frame();
    }

    if peer_count == 0 {
        return;
    }

    // ── Remove resources (scene/io.rs pattern) ──────────────────────────
    let registry = match world.remove_resource::<NetComponentRegistry>() {
        Some(r) => r,
        None => return,
    };
    let snapshot_cache = world
        .remove_resource::<SnapshotCache>()
        .unwrap_or_default();
    let mut net_state = match world.remove_resource::<NetworkState>() {
        Some(s) => s,
        None => {
            world.insert_resource(registry);
            world.insert_resource(snapshot_cache);
            return;
        }
    };
    let mut transport = match world.remove_non_send_resource::<NetworkTransport>() {
        Some(t) => t,
        None => {
            world.insert_resource(registry);
            world.insert_resource(snapshot_cache);
            world.insert_resource(net_state);
            return;
        }
    };
    let mut dirty_tracker = world.remove_resource::<DirtyTracker>().unwrap_or_default();
    let mut dormancy = world.remove_resource::<crate::dormancy::DormancyTracker>().unwrap_or_default();
    let mut priority = world.remove_resource::<crate::priority::ReplicationPriority>().unwrap_or_default();
    let budget = world.remove_resource::<crate::priority::BandwidthBudget>().unwrap_or_default();
    let mut grid = world.remove_resource::<SpatialGrid>().unwrap_or_default();

    // ── 1. Collect Authority entities + detect despawns ──────────────────
    let mut replicated_entities: Vec<(Entity, u64)> = Vec::new();
    let mut despawned_ids: Vec<u64> = Vec::new();

    for (&entity, &net_id) in net_state.entity_to_net.iter() {
        if !world.has_component::<Replicated>(entity) {
            despawned_ids.push(net_id);
            continue;
        }
        if world
            .get::<NetRole>(entity)
            .map(|r| *r == NetRole::Authority)
            .unwrap_or(false)
        {
            replicated_entities.push((entity, net_id));
        }
    }

    // ── 2. Snapshot diff → per-entity change detection ──────────────────
    let mut entity_deltas: std::collections::HashMap<u64, Vec<(String, Vec<u8>)>> = std::collections::HashMap::new();
    let mut new_cache = SnapshotCache::default();
    let mut changed_ids: HashSet<u64> = HashSet::new();
    let all_net_ids: Vec<u64> = replicated_entities.iter().map(|(_, nid)| *nid).collect();

    for (entity, net_id) in &replicated_entities {
        let current = registry.extract_entity(world, *entity);
        let prev = snapshot_cache.cache.get(net_id);

        let changed: Vec<(String, Vec<u8>)> = match prev {
            Some(prev_components) => {
                current
                    .iter()
                    .filter(|(name, data)| {
                        prev_components
                            .iter()
                            .find(|(pn, _)| pn == name)
                            .map(|(_, pd)| pd != data)
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect()
            }
            None => current.clone(), // new entity — send all components
        };

        if !changed.is_empty() {
            changed_ids.insert(*net_id);
            dirty_tracker.mark_dirty(*net_id);
            entity_deltas.insert(*net_id, changed);
        }

        new_cache.cache.insert(*net_id, current);
    }

    // ── 3. Tick dormancy (unchanged entities eventually go dormant) ──────
    dormancy.tick(&changed_ids, &all_net_ids);

    // ── 4. Per-peer send ────────────────────────────────────────────────
    let connected_peers: Vec<u32> = net_state.connected_peers.iter().copied().collect();

    if let Some(t) = transport.as_mut() {
        for &peer_id in &connected_peers {
            let peer_pos = grid.peer_positions.get(&peer_id).copied().unwrap_or(glam::Vec3::ZERO);

            // a. Relevancy: filter entities by distance
            let relevant = grid.query_relevant(peer_id, peer_pos);

            // b. Filter: only changed + not dormant + relevant
            let candidates: Vec<u64> = entity_deltas.keys()
                .filter(|nid| relevant.contains(nid))
                .filter(|nid| !dormancy.is_dormant(**nid))
                .copied()
                .collect();

            if candidates.is_empty() {
                continue;
            }

            // c. Update priorities (distance-based)
            let entity_positions: Vec<(u64, glam::Vec3)> = candidates.iter()
                .filter_map(|nid| {
                    grid.entity_positions.get(nid).map(|pos| (*nid, *pos))
                })
                .collect();
            priority.update(peer_id, &entity_positions, peer_pos);

            // d. Sort by priority (highest first)
            let sorted = priority.sorted_entities(peer_id);

            // e. Estimate serialized sizes → select within bandwidth budget
            let sized: Vec<(u64, usize)> = sorted.iter()
                .filter_map(|(nid, _)| {
                    let delta = entity_deltas.get(nid)?;
                    let size: usize = delta.iter().map(|(n, d)| n.len() + d.len() + 16).sum();
                    Some((*nid, size))
                })
                .collect();
            let selected = budget.select_within_budget(&sized);

            if selected.is_empty() {
                continue;
            }

            // f. Build and send deltas for this peer
            let deltas: Vec<EntityDelta> = selected.iter()
                .filter_map(|nid| {
                    entity_deltas.get(nid).map(|changed| EntityDelta {
                        net_id: *nid,
                        changed_components: changed.clone(),
                    })
                })
                .collect();

            let msg = NetMessage::Delta {
                tick: net_state.tick,
                entities: deltas,
            };
            if let Ok(data) = bincode::serialize(&msg) {
                t.send_to(peer_id, data);
            }

            // g. Reset priority for sent entities
            priority.mark_sent(peer_id, &selected);
        }

        // ── 5. Broadcast despawns to ALL peers (reliable — must not be lost) ──
        if !despawned_ids.is_empty() {
            let msg = NetMessage::Despawn {
                tick: net_state.tick,
                net_ids: despawned_ids.clone(),
            };
            if let Ok(data) = bincode::serialize(&msg) {
                t.broadcast_reliable(data);
            }
        }
    }

    // ── 6. Clean up despawned entities ──────────────────────────────────
    for nid in &despawned_ids {
        if let Some(entity) = net_state.net_to_entity.remove(nid) {
            net_state.entity_to_net.remove(&entity);
        }
        dormancy.remove(*nid);
        new_cache.cache.remove(nid);
    }

    // ── Restore resources ───────────────────────────────────────────────
    world.insert_resource(registry);
    world.insert_resource(new_cache);
    world.insert_resource(net_state);
    world.insert_non_send_resource(transport);
    world.insert_resource(dirty_tracker);
    world.insert_resource(dormancy);
    world.insert_resource(priority);
    world.insert_resource(budget);
    world.insert_resource(grid);
}

/// Receive messages from transport and apply to World.
pub fn network_receive_system(world: &mut World) {
    // Poll connection events and messages in one pass (remove-use-reinsert pattern)
    let (conn_events, messages) = {
        match world.get_non_send_resource_mut::<NetworkTransport>() {
            Some(transport) => match transport.as_mut() {
                Some(t) => (t.poll_connections(), t.poll_messages()),
                None => return,
            },
            None => return,
        }
    };

    // Process connection events
    if !conn_events.is_empty() {
        if let Some(net_state) = world.get_resource_mut::<NetworkState>() {
            for event in &conn_events {
                match event {
                    crate::transport::ConnectionEvent::Disconnected(peer_id) => {
                        log::info!("Peer {} disconnected", peer_id);
                        net_state.connected_peers.remove(peer_id);
                        net_state.peer_count = net_state.connected_peers.len() as u32;
                    }
                    crate::transport::ConnectionEvent::Connected(_) => {
                        // Handled via Handshake message
                    }
                }
            }
        }
        // Emit ECS events for disconnections
        if let Some(events) = world.get_resource_mut::<Events<NetworkEvent>>() {
            for event in &conn_events {
                if let crate::transport::ConnectionEvent::Disconnected(peer_id) = event {
                    events.send(NetworkEvent::PeerDisconnected { peer_id: *peer_id });
                }
            }
        }
    }

    if messages.is_empty() {
        return;
    }

    let registry = match world.remove_resource::<NetComponentRegistry>() {
        Some(r) => r,
        None => return,
    };
    let mut net_state = match world.remove_resource::<NetworkState>() {
        Some(s) => s,
        None => {
            world.insert_resource(registry);
            return;
        }
    };
    let mut transport = match world.remove_non_send_resource::<NetworkTransport>() {
        Some(t) => t,
        None => {
            world.insert_resource(registry);
            world.insert_resource(net_state);
            return;
        }
    };

    let mut new_peers: Vec<(u32, String)> = Vec::new();
    let mut despawned_net_ids: Vec<u64> = Vec::new();
    let mut pending_inputs: Vec<(u32, u64, InputPayload)> = Vec::new();
    let mut pending_rpcs: Vec<(u64, String, Vec<u8>)> = Vec::new();
    let mut pending_pongs: Vec<(u64, u64, u64)> = Vec::new(); // (local_tick, remote_tick, current_tick)
    let mut simulated_proxy_updates: Vec<Entity> = Vec::new();
    let mut autonomous_proxy_updates: Vec<(Entity, u64)> = Vec::new(); // (entity, server_tick)

    for (peer_id, data) in &messages {
        let msg: NetMessage = match bincode::deserialize(data) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("Failed to deserialize NetMessage: {}", e);
                continue;
            }
        };

        match msg {
            NetMessage::Snapshot { entities, .. } => {
                for snap in &entities {
                    let entity = get_or_spawn_net_entity(world, &mut net_state, snap.net_id);
                    registry.insert_components(world, entity, &snap.components);

                    // Client: if entity has NetOwner matching our local_peer_id, upgrade to AutonomousProxy
                    if net_state.role == SessionRole::Client && net_state.local_peer_id != 0 {
                        let owner_id = world.get::<NetOwner>(entity).map(|o| o.0);
                        if owner_id == Some(net_state.local_peer_id) {
                            world.entity_mut(entity).insert(NetRole::AutonomousProxy);
                            log::info!("Snapshot: upgraded entity {:?} to AutonomousProxy (owner={})", entity, net_state.local_peer_id);
                        }
                    }
                }
            }
            NetMessage::Delta { tick: delta_tick, entities } => {
                for delta in &entities {
                    let entity = get_or_spawn_net_entity(world, &mut net_state, delta.net_id);
                    let role = world.get::<NetRole>(entity).copied().unwrap_or(NetRole::SimulatedProxy);

                    match role {
                        NetRole::SimulatedProxy => {
                            registry.insert_components(world, entity, &delta.changed_components);

                            // Client: check if this entity should be AutonomousProxy
                            if net_state.role == SessionRole::Client && net_state.local_peer_id != 0 {
                                let owner_id = world.get::<NetOwner>(entity).map(|o| o.0);
                                if owner_id == Some(net_state.local_peer_id) {
                                    world.entity_mut(entity).insert(NetRole::AutonomousProxy);
                                    log::info!("Delta: upgraded entity {:?} to AutonomousProxy (owner={})", entity, net_state.local_peer_id);
                                    continue; // Don't interpolate our own entity
                                }
                            }

                            simulated_proxy_updates.push(entity);
                        }
                        NetRole::AutonomousProxy => {
                            // For AutonomousProxy: apply non-predicted components,
                            // handle Transform via prediction reconciliation
                            let mut predicted: Vec<(String, Vec<u8>)> = Vec::new();
                            let mut other: Vec<(String, Vec<u8>)> = Vec::new();
                            for comp in &delta.changed_components {
                                if comp.0 == "Transform" {
                                    predicted.push(comp.clone());
                                } else {
                                    other.push(comp.clone());
                                }
                            }
                            if !other.is_empty() {
                                registry.insert_components(world, entity, &other);
                            }
                            if !predicted.is_empty() {
                                autonomous_proxy_updates.push((entity, delta_tick));
                                // Temporarily apply server Transform for reading
                                registry.insert_components(world, entity, &predicted);
                            }
                        }
                        _ => {
                            registry.insert_components(world, entity, &delta.changed_components);
                        }
                    }
                }
            }
            NetMessage::Handshake { client_name } => {
                if net_state.role == SessionRole::Server {
                    if net_state.connected_peers.contains(peer_id) {
                        log::warn!("Duplicate Handshake from peer {}, ignoring", peer_id);
                        continue;
                    }
                    log::info!("Handshake from client '{}' (peer {})", client_name, peer_id);
                    net_state.connected_peers.insert(*peer_id);
                    net_state.peer_count = net_state.connected_peers.len() as u32;
                    new_peers.push((*peer_id, client_name.clone()));

                    let mut snapshots = Vec::new();
                    for (&entity, &net_id) in net_state.entity_to_net.iter() {
                        if !world.has_component::<Replicated>(entity) {
                            continue;
                        }
                        let components = registry.extract_entity(world, entity);
                        snapshots.push(EntitySnapshot {
                            net_id,
                            components,
                        });
                    }

                    // Send full snapshot first (reliable — initial state must arrive)
                    let response = NetMessage::Snapshot {
                        tick: net_state.tick,
                        entities: snapshots,
                    };
                    if let Ok(resp_data) = bincode::serialize(&response) {
                        if let Some(t) = transport.as_mut() {
                            t.send_reliable(*peer_id, resp_data);
                        }
                    }

                    // Then send HandshakeResponse with assigned peer_id (reliable)
                    let hs_response = NetMessage::HandshakeResponse {
                        peer_id: *peer_id,
                        tick: net_state.tick,
                    };
                    if let Ok(hs_data) = bincode::serialize(&hs_response) {
                        if let Some(t) = transport.as_mut() {
                            t.send_reliable(*peer_id, hs_data);
                        }
                    }
                }
            }
            NetMessage::Despawn { net_ids, .. } => {
                for nid in &net_ids {
                    if let Some(entity) = net_state.net_to_entity.remove(nid) {
                        net_state.entity_to_net.remove(&entity);
                        // Check entity is still alive before despawning
                        if world.get::<NetworkId>(entity).is_some() {
                            world.despawn(entity);
                        }
                        despawned_net_ids.push(*nid);
                        log::trace!("Despawned remote entity {:?} (NetworkId {})", entity, nid);
                    }
                }
            }
            NetMessage::HandshakeResponse { peer_id: assigned_id, tick: server_tick } => {
                if net_state.role == SessionRole::Client {
                    net_state.local_peer_id = assigned_id;
                    net_state.tick = server_tick;
                    log::info!(
                        "Handshake response: local_peer_id={}, server_tick={}",
                        assigned_id, server_tick
                    );
                    // Mark local player entity as AutonomousProxy (if exists)
                    let local_entities: Vec<Entity> = net_state.entity_to_net.keys().copied().collect();
                    for entity in local_entities {
                        if world.get::<NetOwner>(entity).map(|o| o.0 == assigned_id).unwrap_or(false) {
                            world.entity_mut(entity).insert(NetRole::AutonomousProxy);
                        }
                    }
                }
            }
            NetMessage::Input { tick: input_tick, data } => {
                if net_state.role == SessionRole::Server {
                    match bincode::deserialize::<InputPayload>(&data) {
                        Ok(payload) => {
                            pending_inputs.push((*peer_id, input_tick, payload));
                        }
                        Err(e) => {
                            log::warn!("Failed to deserialize InputPayload from peer {}: {}", peer_id, e);
                        }
                    }
                }
            }
            NetMessage::Rpc { net_id, method, args } => {
                pending_rpcs.push((net_id, method, args));
            }
            NetMessage::Ping { local_tick, send_time_ms } => {
                if net_state.role == SessionRole::Server {
                    // Respond with Pong
                    let pong = NetMessage::Pong {
                        remote_tick: net_state.tick,
                        local_tick,
                        send_time_ms,
                    };
                    if let Ok(data) = bincode::serialize(&pong) {
                        if let Some(t) = transport.as_mut() {
                            t.send_to(*peer_id, data);
                        }
                    }
                }
            }
            NetMessage::Pong { remote_tick, local_tick, .. } => {
                if net_state.role == SessionRole::Client {
                    pending_pongs.push((local_tick, remote_tick, net_state.tick));
                }
            }
        }
    }

    world.insert_resource(registry);
    world.insert_resource(net_state);
    world.insert_non_send_resource(transport);

    // BUG-7 fix: Register new peers in ConnectionTracker
    if !new_peers.is_empty() {
        let tick = world.get_resource::<NetworkState>().map(|s| s.tick).unwrap_or(0);
        if let Some(ct) = world.get_resource_mut::<ConnectionTracker>() {
            for (pid, name) in &new_peers {
                ct.add_peer(*pid, name.clone(), tick);
            }
        }
    }

    // BUG-8 fix: Touch ConnectionTracker for all peers that sent messages
    {
        let tick = world.get_resource::<NetworkState>().map(|s| s.tick).unwrap_or(0);
        if let Some(ct) = world.get_resource_mut::<ConnectionTracker>() {
            for (peer_id, _) in &messages {
                ct.touch(*peer_id, tick);
            }
        }
    }

    // Update NetInterpolation for SimulatedProxy entities that received Delta updates
    if !simulated_proxy_updates.is_empty() {
        if let Some(acc) = world.remove_resource::<TransformAccessor>() {
            for &entity in &simulated_proxy_updates {
                if let Some((new_pos, new_rot)) = (acc.extract)(world, entity) {
                    if let Some(mut interp) = world.get_mut::<NetInterpolation>(entity) {
                        interp.update_targets(new_pos, new_rot, 1.0);
                    } else {
                        // First Delta for this entity: initialize interpolation at target (no motion)
                        world.entity_mut(entity).insert(NetInterpolation {
                            from_translation: new_pos,
                            to_translation: new_pos,
                            from_rotation: new_rot,
                            to_rotation: new_rot,
                            t: 1.0,
                            duration_ticks: 1.0,
                        });
                    }
                }
            }
            world.insert_resource(acc);
        }
    }

    // Reconcile AutonomousProxy entities: compare server state with prediction
    if !autonomous_proxy_updates.is_empty() {
        let tick_offset = world.get_resource::<TickSync>()
            .map(|ts| ts.tick_offset)
            .unwrap_or(0);

        if let Some(acc) = world.remove_resource::<TransformAccessor>() {
            let mut pred_buf = world.remove_resource::<PredictionBuffer>()
                .unwrap_or_default();

            for &(entity, server_tick) in &autonomous_proxy_updates {
                // Server's authoritative Transform was temporarily written by insert_components
                if let Some((server_pos, _server_rot)) = (acc.extract)(world, entity) {
                    // Map server tick → local tick for prediction lookup
                    let local_tick = (server_tick as i64 - tick_offset).max(0) as u64;

                    // Compare with predicted state at that tick
                    if let Some(error) = pred_buf.reconciliation_error(local_tick, server_pos) {
                        match pred_buf.apply_correction(error) {
                            Some(_snap_error) => {
                                // Snap: keep server position (already written), reset predictions
                                log::debug!("Prediction snap: error={:.2}", error.length());
                                pred_buf.states.clear();
                            }
                            None => {
                                // Blend: restore predicted position, correction offset will blend out
                                if let Some(predicted) = pred_buf.get_state(local_tick) {
                                    let p_translation = predicted.translation;
                                    let p_rotation = predicted.rotation;
                                    (acc.apply)(world, entity, p_translation, p_rotation);
                                }
                            }
                        }
                    }

                    // Acknowledge all predictions up to this tick
                    pred_buf.acknowledge(local_tick);
                }
            }

            world.insert_resource(pred_buf);
            world.insert_resource(acc);
        }
    }

    // Process pending inputs → InputBuffer (after resources restored)
    if !pending_inputs.is_empty() {
        if let Some(input_buf) = world.get_resource_mut::<InputBuffer>() {
            for (pid, tick, payload) in pending_inputs {
                input_buf.push(pid, tick, payload);
            }
        }
    }

    // Process pending RPCs → RpcRegistry dispatch (after resources restored)
    if !pending_rpcs.is_empty() {
        let rpc_reg = world.remove_resource::<RpcRegistry>();
        if let Some(rpc_reg) = rpc_reg {
            for (net_id, method, args) in pending_rpcs {
                let entity = world
                    .get_resource::<NetworkState>()
                    .and_then(|ns| ns.net_to_entity.get(&net_id).copied());
                if let Some(entity) = entity {
                    rpc_reg.dispatch(world, entity, &method, &args);
                } else {
                    log::warn!("RPC target net_id={} not found", net_id);
                }
            }
            world.insert_resource(rpc_reg);
        }
    }

    // Process Pong responses → TickSync
    if !pending_pongs.is_empty() {
        if let Some(tick_sync) = world.get_resource_mut::<TickSync>() {
            for (local_tick, remote_tick, current_tick) in pending_pongs {
                tick_sync.process_pong(local_tick, remote_tick, current_tick);
            }
        }
    }

    // Emit ECS events (after resources restored)
    if !new_peers.is_empty() || !despawned_net_ids.is_empty() {
        if let Some(events) = world.get_resource_mut::<Events<NetworkEvent>>() {
            for (pid, _) in &new_peers {
                events.send(NetworkEvent::PeerConnected { peer_id: *pid });
            }
            for nid in despawned_net_ids {
                events.send(NetworkEvent::EntityDespawned { net_id: nid });
            }
        }
    }
}

/// Tick counter increment (always runs).
pub fn network_tick_system(mut net_state: ResMut<NetworkState>) {
    net_state.tick += 1;
}

/// Find or spawn a local Entity for a given NetworkId.
/// If the previously mapped entity was despawned, removes the stale mapping and respawns.
fn get_or_spawn_net_entity(
    world: &mut World,
    net_state: &mut NetworkState,
    net_id: u64,
) -> Entity {
    if let Some(&entity) = net_state.net_to_entity.get(&net_id) {
        // Guard: check the entity is still alive (could have been despawned)
        if world.get::<NetworkId>(entity).is_some() {
            return entity;
        }
        // Stale mapping — remove it and respawn below
        net_state.net_to_entity.remove(&net_id);
        net_state.entity_to_net.remove(&entity);
        log::warn!("NetworkId({}) mapped to dead entity {:?}, respawning", net_id, entity);
    }
    let entity = world
        .spawn(NetworkId(net_id))
        .insert(Replicated)
        .insert(NetRole::SimulatedProxy)
        .id();
    net_state.register(entity, net_id);
    log::trace!("Spawned remote entity {:?} for NetworkId({})", entity, net_id);
    entity
}

/// Client-side: capture local input and send as NetMessage::Input to server.
/// This is a no-op on the server or when no transport is active.
pub fn network_input_capture_system(world: &mut World) {
    let role = world
        .get_resource::<NetworkState>()
        .map(|s| s.role)
        .unwrap_or(SessionRole::Server);
    if role != SessionRole::Client {
        return;
    }

    let tick = world
        .get_resource::<NetworkState>()
        .map(|s| s.tick)
        .unwrap_or(0);

    // Read the latest input payload (set by the game's input system)
    let payload = match world.remove_resource::<InputPayload>() {
        Some(p) => p,
        None => return,
    };

    let msg = NetMessage::Input {
        tick,
        data: match bincode::serialize(&payload) {
            Ok(d) => d,
            Err(_) => {
                world.insert_resource(payload);
                return;
            }
        },
    };

    if let Some(transport) = world.get_non_send_resource_mut::<NetworkTransport>() {
        if let Some(t) = transport.as_mut() {
            if let Ok(data) = bincode::serialize(&msg) {
                t.send_to(0, data); // send to server (peer 0)
            }
        }
    }

    world.insert_resource(payload);
}

/// Server-side: apply buffered inputs to player entities.
/// Maps peer_id → player entity via NetworkState::peer_to_entity,
/// then updates PlayerController-like components from the InputPayload.
///
/// This system is intentionally generic — it writes the InputPayload as a resource
/// keyed by entity so game systems can consume it. For games that want direct
/// PlayerController integration, see the application-level `network_apply_input_system`.
pub fn network_apply_input_system(world: &mut World) {
    let is_server = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Server)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    let mut input_buf = match world.remove_resource::<InputBuffer>() {
        Some(b) => b,
        None => return,
    };
    let net_state = match world.get_resource::<NetworkState>() {
        Some(s) => s,
        None => {
            world.insert_resource(input_buf);
            return;
        }
    };

    // Collect (entity, payload) pairs to apply
    let mut to_apply: Vec<(Entity, InputPayload)> = Vec::new();
    for (&peer_id, &entity) in net_state.peer_to_entity.iter() {
        if let Some(payload) = input_buf.take_latest(peer_id) {
            to_apply.push((entity, payload));
        }
    }

    world.insert_resource(input_buf);

    // Apply inputs to entities (game-specific logic handled by downstream systems)
    // Store the latest input per entity as a component for game systems to read
    for (entity, payload) in to_apply {
        world.entity_mut(entity).insert(payload);
    }
}

/// Client-side: record prediction state after player movement.
/// Runs after Player stage. Only active for AutonomousProxy entities.
pub fn prediction_record_system(world: &mut World) {
    let is_client = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Client)
        .unwrap_or(false);
    if !is_client {
        return;
    }

    let tick = world
        .get_resource::<NetworkState>()
        .map(|s| s.tick)
        .unwrap_or(0);

    // Collect AutonomousProxy entities' transforms
    let entities: Vec<Entity> = world.alive_entities().into_iter()
        .filter(|&e| {
            world.get::<NetRole>(e).map(|r| *r == NetRole::AutonomousProxy).unwrap_or(false)
        })
        .collect();

    if entities.is_empty() {
        return;
    }

    // Read transform data via TransformAccessor
    let accessor = world.remove_resource::<TransformAccessor>();
    let mut states: Vec<PredictedState> = Vec::new();
    for entity in entities {
        let input = world.get::<InputPayload>(entity)
            .cloned()
            .unwrap_or_default();

        let (translation, rotation) = accessor.as_ref()
            .and_then(|acc| (acc.extract)(world, entity))
            .unwrap_or((glam::Vec3::ZERO, glam::Quat::IDENTITY));

        states.push(PredictedState {
            tick,
            translation,
            rotation,
            velocity: glam::Vec3::ZERO,
            input,
        });
    }
    if let Some(acc) = accessor {
        world.insert_resource(acc);
    }

    if let Some(buf) = world.get_resource_mut::<PredictionBuffer>() {
        for state in states {
            buf.push(state);
        }
    }
}

/// Client-side: reconcile server state with prediction buffer.
/// Runs after NetworkReceive. Compares server authoritative state with
/// predicted state and applies correction.
pub fn prediction_reconcile_system(world: &mut World) {
    let is_client = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Client)
        .unwrap_or(false);
    if !is_client {
        return;
    }

    let dt = world
        .get_resource::<FrameTime>()
        .map(|ft| ft.delta_seconds)
        .unwrap_or(1.0 / 60.0);

    // Blend out correction offset and apply to AutonomousProxy entities
    let offset = {
        match world.get_resource_mut::<PredictionBuffer>() {
            Some(buf) => buf.blend_correction(dt),
            None => return,
        }
    };

    if offset.length_squared() < 0.000001 {
        return;
    }

    // Collect AutonomousProxy entities
    let entities: Vec<Entity> = world.alive_entities().into_iter()
        .filter(|&e| {
            world.get::<NetRole>(e).map(|r| *r == NetRole::AutonomousProxy).unwrap_or(false)
        })
        .collect();

    if entities.is_empty() {
        return;
    }

    // Apply correction offset via TransformAccessor
    if let Some(acc) = world.remove_resource::<TransformAccessor>() {
        for &entity in &entities {
            if let Some((pos, rot)) = (acc.extract)(world, entity) {
                (acc.apply)(world, entity, pos + offset, rot);
            }
        }
        world.insert_resource(acc);
    }
}

/// Interpolation update system — advances SimulatedProxy entities' interpolation
/// and writes interpolated transforms back to the entity's Transform component.
/// Runs before RenderExtract to provide smooth visual representation.
pub fn interpolation_update_system(world: &mut World) {
    let tick_rate = world
        .get_resource::<crate::resources::NetworkConfig>()
        .map(|c| c.tick_rate as f32)
        .unwrap_or(30.0);

    let dt = world
        .get_resource::<FrameTime>()
        .map(|ft| ft.delta_seconds)
        .unwrap_or(1.0 / 60.0);

    let entities: Vec<Entity> = world.alive_entities().into_iter()
        .filter(|&e| world.has_component::<NetInterpolation>(e))
        .collect();

    // Advance interpolation and collect transforms that need write-back
    let mut updates: Vec<(Entity, glam::Vec3, glam::Quat)> = Vec::new();
    for &entity in &entities {
        if let Some(mut interp) = world.get_mut::<NetInterpolation>(entity) {
            let was_interpolating = interp.t < 1.0;
            interp.advance(dt, tick_rate);
            if was_interpolating {
                updates.push((entity, interp.current_translation(), interp.current_rotation()));
            }
        }
    }

    // Write interpolated transforms back to entities via TransformAccessor
    if !updates.is_empty() {
        if let Some(acc) = world.remove_resource::<TransformAccessor>() {
            for &(entity, pos, rot) in &updates {
                (acc.apply)(world, entity, pos, rot);
            }
            world.insert_resource(acc);
        }
    }
}

/// Tick sync system — sends periodic Ping messages for RTT measurement.
/// Client-side only. Sends a Ping every ~30 ticks (1 second at 30Hz).
pub fn tick_sync_system(world: &mut World) {
    let is_client = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Client)
        .unwrap_or(false);
    if !is_client {
        return;
    }

    let tick = world
        .get_resource::<NetworkState>()
        .map(|s| s.tick)
        .unwrap_or(0);

    // Send Ping every 30 ticks
    if tick % 30 != 0 {
        return;
    }

    let msg = NetMessage::Ping {
        local_tick: tick,
        send_time_ms: 0, // could use system clock if needed
    };

    if let Ok(data) = bincode::serialize(&msg) {
        if let Some(transport) = world.get_non_send_resource_mut::<NetworkTransport>() {
            if let Some(t) = transport.as_mut() {
                t.send_to(0, data); // send to server (peer 0)
            }
        }
    }
}

/// Relevancy update system — rebuilds the spatial grid from entity positions.
/// Server-side only. Runs before network_send_system.
///
/// Uses the `position_extractor` callback registered via `set_position_extractor()`
/// to read game-specific Transform components.  If no extractor is set, falls back
/// to zeroed positions (all entities at origin).
pub fn relevancy_update_system(world: &mut World) {
    let is_server = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Server && s.peer_count > 0)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    // Remove resources for mutable access
    let net_state = match world.remove_resource::<NetworkState>() {
        Some(s) => s,
        None => return,
    };
    let mut grid = world.remove_resource::<SpatialGrid>().unwrap_or_default();
    let extractor = world.remove_resource::<PositionExtractor>();

    // Collect replicated entity positions via game-level extractor
    let mut entity_positions: Vec<(u64, glam::Vec3)> = Vec::new();
    for (&entity, &net_id) in net_state.entity_to_net.iter() {
        if !world.has_component::<Replicated>(entity) {
            continue;
        }
        let pos = extractor.as_ref()
            .and_then(|ext| (ext.extract)(world, entity))
            .unwrap_or(glam::Vec3::ZERO);
        entity_positions.push((net_id, pos));
    }

    // Update peer viewpoint positions (from player entities)
    for (&peer_id, &entity) in net_state.peer_to_entity.iter() {
        let pos = extractor.as_ref()
            .and_then(|ext| (ext.extract)(world, entity))
            .unwrap_or(glam::Vec3::ZERO);
        grid.peer_positions.insert(peer_id, pos);
    }

    // Rebuild the spatial grid
    grid.rebuild(&entity_positions);

    // Clean up disconnected peers
    let stale_peers: Vec<u32> = grid.peer_positions.keys()
        .filter(|pid| !net_state.connected_peers.contains(pid))
        .copied()
        .collect();
    for pid in stale_peers {
        grid.remove_peer(pid);
    }

    // Restore resources
    world.insert_resource(net_state);
    world.insert_resource(grid);
    if let Some(ext) = extractor {
        world.insert_resource(ext);
    }
}

/// Heartbeat system — sends a keepalive ping when no other data has been sent recently.
/// Server-side only. Runs in NetworkSend stage.
pub fn network_heartbeat_system(world: &mut World) {
    let is_server = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Server && s.peer_count > 0)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    let tick = world.get_resource::<NetworkState>().map(|s| s.tick).unwrap_or(0);

    let should_send = world.get_resource::<ConnectionTracker>().map(|ct| {
        tick.saturating_sub(ct.last_heartbeat_tick) >= ct.heartbeat_interval_ticks
    }).unwrap_or(false);

    if !should_send {
        return;
    }

    if let Some(ct) = world.get_resource_mut::<ConnectionTracker>() {
        ct.last_heartbeat_tick = tick;
    }

    // Send an empty Delta as heartbeat (peers know we're alive)
    let msg = NetMessage::Delta { tick, entities: vec![] };
    if let Ok(data) = bincode::serialize(&msg) {
        if let Some(transport) = world.get_non_send_resource_mut::<NetworkTransport>() {
            if let Some(t) = transport.as_mut() {
                t.broadcast(data);
            }
        }
    }
}

/// Connection timeout system — disconnects peers that haven't been heard from recently.
/// Server-side only. Runs in NetworkReceive stage (after receive).
pub fn network_connection_timeout_system(world: &mut World) {
    let is_server = world
        .get_resource::<NetworkState>()
        .map(|s| s.role == SessionRole::Server && s.peer_count > 0)
        .unwrap_or(false);
    if !is_server {
        return;
    }

    let tick = world.get_resource::<NetworkState>().map(|s| s.tick).unwrap_or(0);

    let timed_out = world.get_resource::<ConnectionTracker>()
        .map(|ct| ct.timed_out_peers(tick))
        .unwrap_or_default();

    if timed_out.is_empty() {
        return;
    }

    for peer_id in &timed_out {
        log::warn!("Peer {} timed out (no messages for {} ticks)", peer_id,
            world.get_resource::<ConnectionTracker>().map(|ct| ct.timeout_ticks).unwrap_or(0));
    }

    // Disconnect timed-out peers
    if let Some(transport) = world.get_non_send_resource_mut::<NetworkTransport>() {
        if let Some(t) = transport.as_mut() {
            for &peer_id in &timed_out {
                t.disconnect(peer_id);
            }
        }
    }

    // Clean up state
    if let Some(net_state) = world.get_resource_mut::<NetworkState>() {
        for &peer_id in &timed_out {
            net_state.connected_peers.remove(&peer_id);
        }
        net_state.peer_count = net_state.connected_peers.len() as u32;
    }

    if let Some(ct) = world.get_resource_mut::<ConnectionTracker>() {
        for &peer_id in &timed_out {
            ct.remove_peer(peer_id);
        }
    }

    if let Some(input_buf) = world.get_resource_mut::<InputBuffer>() {
        for &peer_id in &timed_out {
            input_buf.remove_peer(peer_id);
        }
    }

    // Emit disconnect events
    if let Some(events) = world.get_resource_mut::<Events<NetworkEvent>>() {
        for &peer_id in &timed_out {
            events.send(NetworkEvent::PeerDisconnected { peer_id });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{Transport, loopback_pair, flush_loopback};

    #[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Pos { x: f32, y: f32 }

    #[derive(Component, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Hp { value: i32 }

    fn setup_world() -> World {
        let mut world = World::new();
        let mut reg = NetComponentRegistry::default();
        reg.register::<Pos>("Pos");
        reg.register::<Hp>("Hp");
        world.insert_resource(reg);
        world.insert_resource(NetworkState::default());
        world.insert_resource(DirtyTracker::default());
        world.insert_resource(SnapshotCache::default());
        world.insert_resource(InputBuffer::default());
        world.insert_resource(RpcRegistry::default());
        world.insert_resource(ConnectionTracker::new());
        // Phase 6 resources
        world.insert_resource(crate::dormancy::DormancyTracker::default());
        world.insert_resource(crate::priority::ReplicationPriority::default());
        world.insert_resource(crate::priority::BandwidthBudget::default());
        world.insert_resource(SpatialGrid::default());
        world
    }

    #[test]
    fn test_network_id_assignment() {
        let mut world = setup_world();
        world.spawn(Replicated).insert(Pos { x: 1.0, y: 2.0 });

        network_id_assignment_system(&mut world);

        let all = world.alive_entities();
        let e = all.iter().find(|&&e| world.has_component::<Replicated>(e)).unwrap();
        let nid = world.get::<NetworkId>(*e).expect("should have NetworkId");
        assert_eq!(nid.0, 1);
        assert_eq!(world.get_resource::<NetworkState>().unwrap().next_network_id, 2);
    }

    #[test]
    fn test_snapshot_diff() {
        let mut server = setup_world();
        server.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        let (server_ep, _client_ep) = loopback_pair();
        server.insert_non_send_resource(NetworkTransport::new(Box::new(server_ep)));

        // Spawn replicated entity with NetworkId already assigned
        let e = server
            .spawn((Replicated, NetworkId(1), Pos { x: 0.0, y: 0.0 }))
            .insert(NetRole::Authority)
            .insert(Hp { value: 100 })
            .id();
        server
            .get_resource_mut::<NetworkState>()
            .unwrap()
            .register(e, 1);

        // First send: all components (no prior cache)
        network_send_system(&mut server);

        // Verify snapshot cache was populated
        let cache = server.get_resource::<SnapshotCache>().unwrap();
        assert!(cache.cache.contains_key(&1));
        assert_eq!(cache.cache[&1].len(), 2); // Pos + Hp

        // Second send: no changes → cache still has same data
        network_send_system(&mut server);
        let cache = server.get_resource::<SnapshotCache>().unwrap();
        assert_eq!(cache.cache[&1].len(), 2);
    }

    #[test]
    fn test_replication_e2e() {
        // Build a Snapshot message with test data
        let pos_bytes = bincode::serialize(&Pos { x: 10.0, y: 20.0 }).unwrap();
        let hp_bytes = bincode::serialize(&Hp { value: 50 }).unwrap();

        let msg = NetMessage::Snapshot {
            tick: 0,
            entities: vec![EntitySnapshot {
                net_id: 1,
                components: vec![
                    ("Pos".to_string(), pos_bytes),
                    ("Hp".to_string(), hp_bytes),
                ],
            }],
        };
        let msg_data = bincode::serialize(&msg).unwrap();

        // Create client world with pre-loaded message via loopback
        let mut client = setup_world();
        client.get_resource_mut::<NetworkState>().unwrap().role = SessionRole::Client;
        client.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        let (mut ep_a, mut ep_b) = loopback_pair();
        ep_a.send_to(1, msg_data);
        flush_loopback(&mut ep_a, &mut ep_b);
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_b)));

        // Client receive
        network_receive_system(&mut client);

        // Verify: client world has a replicated entity with correct data
        let client_ns = client.get_resource::<NetworkState>().unwrap();
        let &client_entity = client_ns
            .net_to_entity
            .get(&1)
            .expect("net_id=1 should exist on client");
        let pos = client.get::<Pos>(client_entity).unwrap();
        assert_eq!(*pos, Pos { x: 10.0, y: 20.0 });
        let hp = client.get::<Hp>(client_entity).unwrap();
        assert_eq!(*hp, Hp { value: 50 });

        // Verify the entity has Replicated + NetworkId + SimulatedProxy
        assert!(client.has_component::<Replicated>(client_entity));
        assert_eq!(client.get::<NetworkId>(client_entity).unwrap().0, 1);
        assert_eq!(
            *client.get::<NetRole>(client_entity).unwrap(),
            NetRole::SimulatedProxy
        );
    }

    #[test]
    fn test_despawn_lifecycle() {
        // Server: spawn → send → despawn → send → verify Despawn message
        let mut server = setup_world();
        server.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        let (server_ep, _client_ep) = loopback_pair();
        server.insert_non_send_resource(NetworkTransport::new(Box::new(server_ep)));

        // Spawn + register
        let e = server
            .spawn((Replicated, NetworkId(1), NetRole::Authority, Pos { x: 1.0, y: 2.0 }))
            .id();
        server.get_resource_mut::<NetworkState>().unwrap().register(e, 1);

        // First send: populates snapshot cache
        network_send_system(&mut server);
        assert!(server.get_resource::<SnapshotCache>().unwrap().cache.contains_key(&1));

        // Despawn the entity
        server.despawn(e);

        // Second send: should detect despawn, clean up cache and mappings
        network_send_system(&mut server);

        // Cache should no longer contain net_id=1
        let cache = server.get_resource::<SnapshotCache>().unwrap();
        assert!(!cache.cache.contains_key(&1));

        // NetworkState mapping should be cleaned up
        let ns = server.get_resource::<NetworkState>().unwrap();
        assert!(!ns.net_to_entity.contains_key(&1));
        assert!(!ns.entity_to_net.contains_key(&e));
    }

    #[test]
    fn test_despawn_receive_e2e() {
        // Client receives a Despawn message → entity should be removed
        let mut client = setup_world();
        client.get_resource_mut::<NetworkState>().unwrap().role = SessionRole::Client;
        client.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        // First: inject a Snapshot to create the entity
        let pos_bytes = bincode::serialize(&Pos { x: 5.0, y: 6.0 }).unwrap();
        let snap_msg = NetMessage::Snapshot {
            tick: 0,
            entities: vec![EntitySnapshot {
                net_id: 42,
                components: vec![("Pos".to_string(), pos_bytes)],
            }],
        };
        let snap_data = bincode::serialize(&snap_msg).unwrap();

        let (mut ep_a, mut ep_b) = loopback_pair();
        ep_a.send_to(1, snap_data);
        flush_loopback(&mut ep_a, &mut ep_b);
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_b)));

        network_receive_system(&mut client);

        // Verify entity exists
        let entity = *client
            .get_resource::<NetworkState>()
            .unwrap()
            .net_to_entity
            .get(&42)
            .expect("entity should exist");
        assert!(client.get::<Pos>(entity).is_some());

        // Now send Despawn message
        let despawn_msg = NetMessage::Despawn {
            tick: 1,
            net_ids: vec![42],
        };
        let despawn_data = bincode::serialize(&despawn_msg).unwrap();

        let (mut ep_c, mut ep_d) = loopback_pair();
        ep_c.send_to(1, despawn_data);
        flush_loopback(&mut ep_c, &mut ep_d);
        // Replace transport with new one containing despawn message
        client.remove_non_send_resource::<NetworkTransport>();
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_d)));

        network_receive_system(&mut client);

        // Entity should be gone
        let ns = client.get_resource::<NetworkState>().unwrap();
        assert!(!ns.net_to_entity.contains_key(&42));
        // The entity handle should point to a dead entity
        assert!(client.get::<Pos>(entity).is_none());
    }

    // ========== Phase 3 Tests ==========

    #[test]
    fn test_input_send_receive() {
        // Directly inject an Input message into the server's transport and verify
        // it gets deserialized into the InputBuffer.
        let mut server = setup_world();
        server.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;
        server.get_resource_mut::<NetworkState>().unwrap().tick = 10;

        let payload = InputPayload {
            move_direction: [1.0, 0.0, 0.0],
            is_running: true,
            ..Default::default()
        };
        let input_msg = NetMessage::Input {
            tick: 10,
            data: bincode::serialize(&payload).unwrap(),
        };
        let msg_data = bincode::serialize(&input_msg).unwrap();

        // Use loopback: client sends to server
        let (mut server_ep, mut client_ep) = loopback_pair();
        client_ep.send_to(0, msg_data);
        flush_loopback(&mut server_ep, &mut client_ep);

        server.insert_non_send_resource(NetworkTransport::new(Box::new(server_ep)));
        network_receive_system(&mut server);

        // Verify InputBuffer has the payload from peer 1
        let buf = server.get_resource::<InputBuffer>().unwrap();
        let latest = buf.peek_latest(1).expect("should have input from peer 1");
        assert!((latest.move_direction[0] - 1.0).abs() < 0.001);
        assert!(latest.is_running);
    }

    #[test]
    fn test_input_apply() {
        let mut world = setup_world();
        world.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        // Spawn a player entity and map peer 1 to it
        let entity = world.spawn(Pos { x: 0.0, y: 0.0 }).id();
        world.get_resource_mut::<NetworkState>().unwrap().peer_to_entity.insert(1, entity);

        // Push input into buffer
        world.get_resource_mut::<InputBuffer>().unwrap().push(1, 10, InputPayload {
            move_direction: [0.0, 1.0, 0.0],
            jump: true,
            ..Default::default()
        });

        // Apply
        network_apply_input_system(&mut world);

        // Verify: entity should have InputPayload component
        let applied = world.get::<InputPayload>(entity).expect("should have InputPayload");
        assert!(applied.jump);
        assert!((applied.move_direction[1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_handshake_response() {
        // Server receives Handshake → sends HandshakeResponse
        // Client receives HandshakeResponse → sets local_peer_id and tick
        let mut client = setup_world();
        client.get_resource_mut::<NetworkState>().unwrap().role = SessionRole::Client;

        let hs_resp = NetMessage::HandshakeResponse { peer_id: 42, tick: 100 };
        let data = bincode::serialize(&hs_resp).unwrap();

        let (mut ep_a, mut ep_b) = loopback_pair();
        ep_a.send_to(1, data);
        flush_loopback(&mut ep_a, &mut ep_b);
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_b)));

        network_receive_system(&mut client);

        let ns = client.get_resource::<NetworkState>().unwrap();
        assert_eq!(ns.local_peer_id, 42);
        assert_eq!(ns.tick, 100);
    }

    #[test]
    fn test_rpc_dispatch_via_receive() {
        use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut world = setup_world();
        world.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        // Register RPC handler
        world.get_resource_mut::<RpcRegistry>().unwrap().register("test_rpc", move |_w, _e, args| {
            let val: u32 = bincode::deserialize(args).unwrap_or(0);
            counter_clone.fetch_add(val, Ordering::SeqCst);
        });

        // Spawn a replicated entity
        let e = world.spawn((Replicated, NetworkId(1), NetRole::Authority)).id();
        world.get_resource_mut::<NetworkState>().unwrap().register(e, 1);

        // Inject RPC message
        let rpc_msg = NetMessage::Rpc {
            net_id: 1,
            method: "test_rpc".to_string(),
            args: bincode::serialize(&10u32).unwrap(),
        };
        let msg_data = bincode::serialize(&rpc_msg).unwrap();

        let (mut ep_a, mut ep_b) = loopback_pair();
        ep_a.send_to(1, msg_data);
        flush_loopback(&mut ep_a, &mut ep_b);
        world.insert_non_send_resource(NetworkTransport::new(Box::new(ep_b)));

        network_receive_system(&mut world);

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_solo_zero_overhead() {
        // With peer_count==0, send/receive/input systems should be no-ops
        let mut world = setup_world();
        assert_eq!(world.get_resource::<NetworkState>().unwrap().peer_count, 0);

        // These should all return immediately without doing anything
        network_send_system(&mut world);
        network_input_capture_system(&mut world);
        network_apply_input_system(&mut world);
    }

    // ========== Phase 6 Tests ==========

    #[test]
    fn test_per_peer_send() {
        // Two peers at different positions receive different entity sets
        // based on relevancy (cull_distance = 100).
        let mut server = setup_world();
        {
            let ns = server.get_resource_mut::<NetworkState>().unwrap();
            ns.peer_count = 2;
            ns.connected_peers.insert(1);
            ns.connected_peers.insert(2);
        }

        // Configure small cull distance for testing
        server.get_resource_mut::<SpatialGrid>().unwrap().config.cull_distance = 100.0;
        server.get_resource_mut::<SpatialGrid>().unwrap().config.hysteresis_frames = 0;

        let (server_ep, _) = loopback_pair();
        server.insert_non_send_resource(NetworkTransport::new(Box::new(server_ep)));

        // Spawn 3 entities at different locations
        let e1 = server.spawn((Replicated, NetworkId(1), NetRole::Authority, Pos { x: 0.0, y: 0.0 })).id();
        let e2 = server.spawn((Replicated, NetworkId(2), NetRole::Authority, Pos { x: 50.0, y: 0.0 })).id();
        let e3 = server.spawn((Replicated, NetworkId(3), NetRole::Authority, Pos { x: 500.0, y: 0.0 })).id();
        {
            let ns = server.get_resource_mut::<NetworkState>().unwrap();
            ns.register(e1, 1);
            ns.register(e2, 2);
            ns.register(e3, 3);
        }

        // Populate spatial grid: peer 1 at origin, peer 2 at (500, 0, 0)
        {
            let grid = server.get_resource_mut::<SpatialGrid>().unwrap();
            grid.rebuild(&[
                (1, glam::Vec3::new(0.0, 0.0, 0.0)),
                (2, glam::Vec3::new(50.0, 0.0, 0.0)),
                (3, glam::Vec3::new(500.0, 0.0, 0.0)),
            ]);
            grid.peer_positions.insert(1, glam::Vec3::ZERO);
            grid.peer_positions.insert(2, glam::Vec3::new(500.0, 0.0, 0.0));
        }

        // First send: per-peer filtering should work
        network_send_system(&mut server);

        // Verify snapshot cache was populated for all entities
        let cache = server.get_resource::<SnapshotCache>().unwrap();
        assert!(cache.cache.contains_key(&1));
        assert!(cache.cache.contains_key(&2));
        assert!(cache.cache.contains_key(&3));

        // Verify dormancy tracker received the entities
        let dormancy = server.get_resource::<crate::dormancy::DormancyTracker>().unwrap();
        // After one tick with changes, entities should NOT be dormant
        assert!(!dormancy.is_dormant(1));
        assert!(!dormancy.is_dormant(2));
        assert!(!dormancy.is_dormant(3));

        // Verify all Phase 6 resources were properly restored
        assert!(server.get_resource::<DirtyTracker>().is_some());
        assert!(server.get_resource::<crate::dormancy::DormancyTracker>().is_some());
        assert!(server.get_resource::<crate::priority::ReplicationPriority>().is_some());
        assert!(server.get_resource::<crate::priority::BandwidthBudget>().is_some());
        assert!(server.get_resource::<SpatialGrid>().is_some());
    }

    #[test]
    fn test_dormancy_integration() {
        // Verify that unchanged entities eventually become dormant
        // and are excluded from per-peer send.
        let mut server = setup_world();
        {
            let ns = server.get_resource_mut::<NetworkState>().unwrap();
            ns.peer_count = 1;
            ns.connected_peers.insert(1);
        }

        // Set dormancy threshold to 3 ticks for fast testing
        server.get_resource_mut::<crate::dormancy::DormancyTracker>().unwrap().dormancy_threshold = 3;

        let (server_ep, _) = loopback_pair();
        server.insert_non_send_resource(NetworkTransport::new(Box::new(server_ep)));

        let e = server.spawn((Replicated, NetworkId(1), NetRole::Authority, Pos { x: 0.0, y: 0.0 })).id();
        server.get_resource_mut::<NetworkState>().unwrap().register(e, 1);

        // Populate grid
        {
            let grid = server.get_resource_mut::<SpatialGrid>().unwrap();
            grid.rebuild(&[(1, glam::Vec3::ZERO)]);
            grid.peer_positions.insert(1, glam::Vec3::ZERO);
        }

        // First send: entity is new → changed
        network_send_system(&mut server);
        assert!(!server.get_resource::<crate::dormancy::DormancyTracker>().unwrap().is_dormant(1));

        // Re-populate grid (it gets consumed by send)
        server.get_resource_mut::<SpatialGrid>().unwrap().rebuild(&[(1, glam::Vec3::ZERO)]);
        server.get_resource_mut::<SpatialGrid>().unwrap().peer_positions.insert(1, glam::Vec3::ZERO);

        // 3 more sends without changes → dormancy threshold reached
        for _ in 0..3 {
            network_send_system(&mut server);
            server.get_resource_mut::<SpatialGrid>().unwrap().rebuild(&[(1, glam::Vec3::ZERO)]);
            server.get_resource_mut::<SpatialGrid>().unwrap().peer_positions.insert(1, glam::Vec3::ZERO);
        }

        assert!(server.get_resource::<crate::dormancy::DormancyTracker>().unwrap().is_dormant(1));
    }

    #[test]
    fn test_simulated_proxy_interpolation() {
        // Client receives two Delta messages → NetInterpolation should interpolate between them.
        let mut client = setup_world();
        client.get_resource_mut::<NetworkState>().unwrap().role = SessionRole::Client;
        client.get_resource_mut::<NetworkState>().unwrap().peer_count = 1;

        // Register TransformAccessor using Pos component for testing
        client.insert_resource(TransformAccessor::new(
            |world, entity| {
                world.get::<Pos>(entity).map(|p| {
                    (glam::Vec3::new(p.x, p.y, 0.0), glam::Quat::IDENTITY)
                })
            },
            |world, entity, pos, _rot| {
                if let Some(mut p) = world.get_mut::<Pos>(entity) {
                    p.x = pos.x;
                    p.y = pos.y;
                }
            },
        ));

        // First Delta: entity at (0, 0)
        let pos_bytes = bincode::serialize(&Pos { x: 0.0, y: 0.0 }).unwrap();
        let msg1 = NetMessage::Delta {
            tick: 1,
            entities: vec![EntityDelta {
                net_id: 1,
                changed_components: vec![("Pos".to_string(), pos_bytes)],
            }],
        };
        let msg1_data = bincode::serialize(&msg1).unwrap();

        let (mut ep_a, mut ep_b) = loopback_pair();
        ep_a.send_to(1, msg1_data);
        flush_loopback(&mut ep_a, &mut ep_b);
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_b)));

        network_receive_system(&mut client);

        // Entity should exist with NetInterpolation (first update: from=to, t=1.0)
        let entity = *client.get_resource::<NetworkState>().unwrap()
            .net_to_entity.get(&1).expect("entity should exist");
        let interp = client.get::<NetInterpolation>(entity).expect("should have NetInterpolation");
        assert_eq!(interp.t, 1.0); // fully arrived
        assert!((interp.to_translation.x - 0.0).abs() < 0.01);

        // Second Delta: entity moves to (10, 0)
        let pos_bytes2 = bincode::serialize(&Pos { x: 10.0, y: 0.0 }).unwrap();
        let msg2 = NetMessage::Delta {
            tick: 2,
            entities: vec![EntityDelta {
                net_id: 1,
                changed_components: vec![("Pos".to_string(), pos_bytes2)],
            }],
        };
        let msg2_data = bincode::serialize(&msg2).unwrap();

        let (mut ep_c, mut ep_d) = loopback_pair();
        ep_c.send_to(1, msg2_data);
        flush_loopback(&mut ep_c, &mut ep_d);
        client.remove_non_send_resource::<NetworkTransport>();
        client.insert_non_send_resource(NetworkTransport::new(Box::new(ep_d)));

        network_receive_system(&mut client);

        // NetInterpolation should interpolate from (0,0) to (10,0), t=0
        let interp = client.get::<NetInterpolation>(entity).expect("should have NetInterpolation");
        assert_eq!(interp.t, 0.0); // just received new target
        assert!((interp.from_translation.x - 0.0).abs() < 0.01); // from old position
        assert!((interp.to_translation.x - 10.0).abs() < 0.01); // to new position

        // Run interpolation update system — should advance t and write back
        interpolation_update_system(&mut client);

        // t should have advanced (dt=1/60, tick_rate=30, duration=1 tick → duration_seconds=1/30)
        // advance = (1/60) / (1/30) = 0.5
        let interp = client.get::<NetInterpolation>(entity).unwrap();
        assert!((interp.t - 0.5).abs() < 0.05);

        // Pos should reflect interpolated position (~5.0)
        let pos = client.get::<Pos>(entity).unwrap();
        assert!((pos.x - 5.0).abs() < 0.5, "expected ~5.0, got {}", pos.x);
    }
}
