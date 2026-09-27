//! Lead-anchored recovery for a containment Tank pair that has split apart: freeze the lead Tank
//! and route only the rear Tank to a point just behind it.

use super::*;

const CONTAINMENT_TANK_CATCH_UP_BEHIND_TILES: f32 = 1.5;

pub(super) fn frontmost_unit_id(
    observation: &AiObservation,
    unit_ids: &[u32],
    toward_objective: (f32, f32),
) -> Option<u32> {
    observation
        .owned
        .iter()
        .filter(|unit| unit_ids.contains(&unit.id))
        .max_by(|left, right| {
            let left_progress = left.x * toward_objective.0 + left.y * toward_objective.1;
            let right_progress = right.x * toward_objective.0 + right.y * toward_objective.1;
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| unit.id)
}

pub(super) fn rearmost_unit_id(
    observation: &AiObservation,
    unit_ids: &[u32],
    toward_objective: (f32, f32),
) -> Option<u32> {
    observation
        .owned
        .iter()
        .filter(|unit| unit_ids.contains(&unit.id))
        .min_by(|left, right| {
            let left_progress = left.x * toward_objective.0 + left.y * toward_objective.1;
            let right_progress = right.x * toward_objective.0 + right.y * toward_objective.1;
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| unit.id)
}

pub(super) fn unit_position(observation: &AiObservation, unit_id: u32) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .find(|unit| unit.id == unit_id)
        .map(|unit| (unit.x, unit.y))
}

pub(super) fn tank_catch_up_point(
    lead_position: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let direction = normalized_direction(own_base, objective)?;
    let distance = CONTAINMENT_TANK_CATCH_UP_BEHIND_TILES * map.tile_size as f32;
    Some(clamp_to_map(
        (
            lead_position.0 - direction.0 * distance,
            lead_position.1 - direction.1 * distance,
        ),
        map,
    ))
}

pub(super) fn tank_catch_up_point_on_route(
    analysis: &AiMapAnalysis,
    rear_position: (f32, f32),
    lead_position: (f32, f32),
) -> Option<(f32, f32)> {
    // The final route point is the lead Tank's tile. Select the preceding passable tile so the
    // rear Tank closes to formation distance without attempting to occupy the lead's space.
    let route = analysis.compact_group_route(rear_position, lead_position, 1);
    (route.len() >= 2).then(|| route[route.len() - 2])
}
