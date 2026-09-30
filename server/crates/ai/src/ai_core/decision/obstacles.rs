//! Clearing Tank Traps. A completed trap is neutral and inert: it blocks vehicles (two traps one
//! tile apart close the gap between them), never blocks shots, and is never fired on unless units
//! are ordered to clear the area around it; then they shoot every trap within four tiles, ahead of
//! ordinary targets. Jeff clears the traps that stand in its Tanks' way, and only while nothing
//! hostile is near the trap or the units clearing it. Traps anywhere else are left alone: some may
//! be keeping enemy armor out of Jeff's own base. A push never clears a trap on Jeff's own side of
//! the map (nearer Jeff's HQ than the enemy base): it goes round it. On Schone Tage pushes used to
//! cut through their own side's trap lines, and one opened the wall beside Jeff's main.

use super::geometry::{dist2, normalized_direction, squared, tile_center};
use super::*;

/// A marching push clears traps this far ahead of its Tanks and this close to its line of march.
const PUSH_TRAP_AHEAD_TILES: f32 = 10.0;
const PUSH_TRAP_LANE_TILES: f32 = 3.0;
/// Tanks at home clear traps this close to the route between the main and the enemy, out to this
/// far from the HQ: the main's own way out.
const ROUTE_TRAP_LANE_TILES: f32 = 3.0;
const ROUTE_TRAP_REACH_TILES: f32 = 32.0;
/// Nothing hostile may be this close to the trap or the units that would clear it.
const TRAP_SAFE_TILES: f32 = 16.0;
/// At most this many home Tanks go to clear a trap.
const HOME_TRAP_CLEARERS: usize = 2;
/// A clearing order stands this long before it is given again.
pub(super) const TRAP_ORDER_REFRESH_TICKS: u32 = config::TICK_HZ * 2;

/// A hostile combat unit is within `tiles` of `point`.
fn hostile_near(observation: &AiObservation, point: (f32, f32), tiles: f32) -> bool {
    let reach2 = squared(tiles * observation.map.tile_size as f32);
    observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && enemy.kind.is_unit()
            && !matches!(enemy.kind, EntityKind::Worker | EntityKind::ScoutPlane)
            && dist2(enemy.x, enemy.y, point.0, point.1) <= reach2
    })
}

fn center_of(observation: &AiObservation, units: &[u32]) -> Option<(f32, f32)> {
    let (sum_x, sum_y, count) = observation
        .owned
        .iter()
        .filter(|unit| units.contains(&unit.id))
        .fold((0.0, 0.0, 0usize), |(x, y, n), unit| {
            (x + unit.x, y + unit.y, n + 1)
        });
    (count > 0).then(|| (sum_x / count as f32, sum_y / count as f32))
}

/// A safe trap across a marching push's way: ahead of its Tanks toward `destination`, near their
/// line of march, on the enemy's side of the map (nearer `enemy_base` than Jeff's HQ), with
/// nothing hostile near it or them.
pub(super) fn trap_across_push(
    observation: &AiObservation,
    tanks: &[u32],
    destination: (f32, f32),
    enemy_base: (f32, f32),
) -> Option<u32> {
    if observation.visible_tank_traps.is_empty() {
        return None;
    }
    let center = center_of(observation, tanks)?;
    if hostile_near(observation, center, TRAP_SAFE_TILES) {
        return None;
    }
    let dir = normalized_direction(center, destination)?;
    let ts = observation.map.tile_size as f32;
    let hq = tile_center(observation.own_start_tile, observation.map.tile_size);
    observation
        .visible_tank_traps
        .iter()
        .filter_map(|trap| {
            let rel = (trap.x - center.0, trap.y - center.1);
            let along = rel.0 * dir.0 + rel.1 * dir.1;
            let lateral = (rel.0 * dir.1 - rel.1 * dir.0).abs();
            let enemy_side = dist2(trap.x, trap.y, enemy_base.0, enemy_base.1)
                < dist2(trap.x, trap.y, hq.0, hq.1);
            (along > 0.0
                && along <= PUSH_TRAP_AHEAD_TILES * ts
                && lateral <= PUSH_TRAP_LANE_TILES * ts
                && enemy_side
                && !hostile_near(observation, (trap.x, trap.y), TRAP_SAFE_TILES))
            .then_some((trap.id, along))
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _)| id)
}

/// Home Tanks clear a safe trap on the main's way out: near the route between the main and the
/// enemy, within `ROUTE_TRAP_REACH_TILES` of the HQ, while the base is not under attack. Returns the
/// Tanks sent.
pub(super) fn clear_route_traps(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    claimed: &BTreeSet<u32>,
) -> Option<Vec<u32>> {
    if observation.visible_tank_traps.is_empty()
        || defense::local_defense_contact(observation).is_some()
    {
        return None;
    }
    let route = map_analysis?.base_route_tiles(observation.player_id)?;
    let ts = observation.map.tile_size as f32;
    let hq = tile_center(observation.own_start_tile, observation.map.tile_size);
    let lane2 = squared(ROUTE_TRAP_LANE_TILES * ts);
    let trap = observation
        .visible_tank_traps
        .iter()
        .filter(|trap| {
            dist2(trap.x, trap.y, hq.0, hq.1) <= squared(ROUTE_TRAP_REACH_TILES * ts)
                && route.iter().any(|tile| {
                    let point = tile_center((tile.x, tile.y), observation.map.tile_size);
                    dist2(trap.x, trap.y, point.0, point.1) <= lane2
                })
                && !hostile_near(observation, (trap.x, trap.y), TRAP_SAFE_TILES)
        })
        .min_by(|left, right| {
            dist2(left.x, left.y, hq.0, hq.1)
                .total_cmp(&dist2(right.x, right.y, hq.0, hq.1))
                .then_with(|| left.id.cmp(&right.id))
        })?;
    if memory.trap_order.is_some_and(|(id, tick)| {
        id == trap.id && observation.tick.saturating_sub(tick) < TRAP_ORDER_REFRESH_TICKS
    }) {
        return None;
    }
    let mut clearers: Vec<&AiEntitySummary> = later_bases::main_tank_ids(observation, memory)
        .into_iter()
        .filter(|id| Some(*id) != memory.home_defensive_tank && !claimed.contains(id))
        .filter_map(|id| observation.owned.iter().find(|unit| unit.id == id))
        .filter(|unit| !hostile_near(observation, (unit.x, unit.y), TRAP_SAFE_TILES))
        .collect();
    clearers.sort_by(|left, right| {
        dist2(left.x, left.y, trap.x, trap.y)
            .total_cmp(&dist2(right.x, right.y, trap.x, trap.y))
            .then_with(|| left.id.cmp(&right.id))
    });
    let sent = actions::clear_obstacle_area(
        actions,
        clearers.iter().take(HOME_TRAP_CLEARERS).map(|unit| unit.id),
        trap.id,
    )?;
    memory.trap_order = Some((trap.id, observation.tick));
    Some(sent)
}

#[cfg(test)]
#[path = "obstacles_tests.rs"]
mod tests;
