use super::*;

pub(super) fn uses_home_rifle_coverage(profile_id: &str) -> bool {
    matches!(
        profile_id,
        JEFFS_AI_ID
            | JEFFS_AI_BETA_ID
            | JEFFS_AI_PRE_DEFENSE_ENVELOPE_ID
            | JEFFS_AI_PRE_TANK_CATCHUP_ID
    )
}

pub(super) fn uses_current_jeff_defense(profile_id: &str) -> bool {
    matches!(
        profile_id,
        JEFFS_AI_ID | JEFFS_AI_BETA_ID | JEFFS_AI_PRE_TANK_CATCHUP_ID
    )
}

/// Jeff-only mirror of the server's build-site rule for resource nodes: a node body is a half-tile
/// circle and any footprint it touches is rejected. The shared placement query checks only the
/// node's center tile, so on Classic Jeff kept choosing a natural the server refused because two
/// Steel centers sat just outside the footprint edge. Pump Jacks and Steel Mines are exempt, as
/// in the simulation. Kept out of the shared query so other profiles are unaffected.
pub(super) fn resource_body_blocks_site(
    observation: &AiObservation,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    if matches!(building, EntityKind::PumpJack | EntityKind::SteelMine) {
        return false;
    }
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    let ts = observation.map.tile_size as f32;
    let footprint = (
        tile_x as f32 * ts,
        tile_y as f32 * ts,
        tile_x.saturating_add(stats.foot_w) as f32 * ts,
        tile_y.saturating_add(stats.foot_h) as f32 * ts,
    );
    observation.resources.iter().any(|resource| {
        crate::sdk::unit_circle_touches_rect((resource.x, resource.y), ts * 0.5, footprint)
    })
}

/// Jeff's producers send fresh combat units to a safe forward staging point immediately.
/// The normal frontal and defense planners remain authoritative and can redirect them on
/// the next think; this route only removes the idle interval at the production building.
pub(super) fn production_rally(observation: &AiObservation, facts: &AiFacts) -> Option<(f32, f32)> {
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(own_base, (enemy_base.x, enemy_base.y))?;
    let forward_distance = observation.map.tile_size as f32 * 8.0;
    Some(clamp_to_map(
        (
            own_base.0 + direction.0 * forward_distance,
            own_base.1 + direction.1 * forward_distance,
        ),
        observation.map,
    ))
}

/// Riflemen are permanent home-screen units. Rally them to the base-centric defensive anchor so
/// they do not walk through the forward army staging lane before receiving a stable slot.
pub(super) fn rifleman_home_rally(
    observation: &AiObservation,
    facts: &AiFacts,
) -> Option<(f32, f32)> {
    let anchor = defense::main_steel_cluster_center(observation)
        .unwrap_or_else(|| tile_center(observation.own_start_tile, observation.map.tile_size));
    let enemy_base = facts.nearest_public_enemy_base?;
    let direction = normalized_direction(anchor, (enemy_base.x, enemy_base.y))?;
    let distance = observation.map.tile_size as f32 * 3.5;
    Some(clamp_to_map(
        (
            anchor.0 + direction.0 * distance,
            anchor.1 + direction.1 * distance,
        ),
        observation.map,
    ))
}
