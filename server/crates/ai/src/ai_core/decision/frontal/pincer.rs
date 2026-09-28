//! Two-pronged pushes. When Jeff has the Tanks for two groups beyond its home reserve, a second
//! Scout Car, a main that holds its reserve, and two separate ways to reach the target, the push
//! splits: each group marches to a staging point on its own side of the target and waits there
//! for the other, then both close in together. A group that makes contact before its partner is in
//! place creeps forward in short steps rather than pressing in alone. Without all of that the push
//! stays one group, as before.
//!
//! The partner group runs through the same push code as the main one: its state is swapped into
//! `memory.containment` while it is driven, and the main's waits in `memory.partner_push`.

use super::*;
use crate::ai_core::decision::geometry::squared;
use crate::ai_core::decision::memory::ContainmentPush;

/// Each group needs at least this many Tanks.
pub(super) const PRONG_MIN_TANKS: usize = 4;
/// Angles either side of the line from the target back toward Jeff's base tried for the two
/// sides, narrowest first: a slight shift left and right keeps the way round short.
const SIDE_ANGLES_DEGREES: [f32; 3] = [35.0, 45.0, 55.0];
/// Staging points sit this much farther from the target than the attack points.
const STAGING_EXTRA_TILES: f32 = 8.0;
/// The group faces the target from this much farther out along its side.
const APPROACH_EXTRA_TILES: f32 = 10.0;
/// A group's route to its staging point may not pass this close to the target or the enemy main.
const ROUTE_CLEARANCE_OF_TARGET_TILES: f32 = 10.0;
const ROUTE_CLEARANCE_OF_ENEMY_MAIN_TILES: f32 = 18.0;
/// A side point may move this far to find open ground.
const POINT_SNAP_TILES: i32 = 3;
/// Once one group is in place it waits for the other while the other keeps closing in. When the
/// late group has made no progress for `STALL_TICKS`, or `SYNC_LIMIT_TICKS` have passed, both go in
/// if it is within `NEAR_TILES` of its point; otherwise the pincer is called off and the group in
/// place carries on as one push. A long way round is slow for a push that stops to fight.
const STALL_TICKS: u32 = config::TICK_HZ * 30;
const SYNC_LIMIT_TICKS: u32 = config::TICK_HZ * 180;
const NEAR_TILES: f32 = 25.0;
/// Closing in by at least this much counts as progress.
const PROGRESS_TILES: f32 = 2.0;
/// A group in contact before its partner is in place steps this far forward this often.
const CREEP_STEP_TILES: f32 = 2.0;
const CREEP_INTERVAL_TICKS: u32 = config::TICK_HZ * 5;
/// Contact only means a group has reached the target, and only then does it creep, within this
/// much beyond its staging distance. Fighting on the way there is ordinary stop-and-fight.
const ENGAGED_MARGIN_TILES: f32 = 6.0;
/// The target point follows the nearest steel still in the ground; a shift this small is the same
/// target, not a new one.
const OBJECTIVE_SHIFT_TILES: f32 = 8.0;
/// Approach lanes to a target are worked out again after this long.
const LANE_CACHE_TICKS: u32 = config::TICK_HZ * 30;

type WorldPoint = (i32, i32);

fn stored(point: (f32, f32)) -> WorldPoint {
    (point.0.round() as i32, point.1.round() as i32)
}

fn world(point: WorldPoint) -> (f32, f32) {
    (point.0 as f32, point.1 as f32)
}

/// One side of a pincer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PincerSide {
    staging: WorldPoint,
    attack: WorldPoint,
    /// A point farther out on this side, so the group lines up across its own approach.
    approach_from: WorldPoint,
}

/// Approach lanes last worked out for a target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PincerLanes {
    objective: WorldPoint,
    tick: u32,
    sides: Option<[PincerSide; 2]>,
}

/// A two-pronged push under way. Index 0 is the main group (`memory.containment`), 1 the partner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pincer {
    objective: WorldPoint,
    sides: [PincerSide; 2],
    /// Where each group waits for the other: its staging point, moved forward by creep steps.
    hold: [WorldPoint; 2],
    arrived: [bool; 2],
    first_arrival_tick: Option<u32>,
    advancing: bool,
    last_creep_tick: [Option<u32>; 2],
    /// Each group's closest approach to its waiting point, in tiles, and when it last improved.
    closest: [Option<i32>; 2],
    last_progress_tick: [u32; 2],
}

/// Where the push being driven goes this decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ProngOrders {
    pub(super) destination: (f32, f32),
    pub(super) approach_from: (f32, f32),
    /// Step forward now even though the group is in contact.
    pub(super) creep_step: bool,
}

fn rotate(vector: (f32, f32), degrees: f32) -> (f32, f32) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (
        vector.0 * cos - vector.1 * sin,
        vector.0 * sin + vector.1 * cos,
    )
}

/// Two sides to approach `objective` from, each with open ground at its staging and attack points
/// and a route from Jeff's base that stays clear of the target and the enemy main. `None` when the
/// terrain offers no such pair (Crossroads' single way in, most River crossings).
pub(super) fn pincer_sides(
    analysis: &AiMapAnalysis,
    observation: &AiObservation,
    own_base: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    standoff_tiles: f32,
) -> Option<[PincerSide; 2]> {
    let ts = observation.map.tile_size as f32;
    let back = normalized_direction(objective, own_base)?;
    let side = |angle: f32| -> Option<PincerSide> {
        let dir = rotate(back, angle);
        let at = |tiles: f32| {
            clamp_to_map(
                (
                    objective.0 + dir.0 * tiles * ts,
                    objective.1 + dir.1 * tiles * ts,
                ),
                observation.map,
            )
        };
        let attack =
            analysis.open_ground_near(own_base, at(standoff_tiles), 2, POINT_SNAP_TILES)?;
        let staging = analysis.open_ground_near(
            own_base,
            at(standoff_tiles + STAGING_EXTRA_TILES),
            2,
            POINT_SNAP_TILES,
        )?;
        let route = analysis.compact_group_route(own_base, staging, 1);
        // The route search falls back to the bare destination when there is no path.
        if route.len() < 2 {
            return None;
        }
        let target_clearance2 = squared(ROUTE_CLEARANCE_OF_TARGET_TILES * ts);
        let main_clearance2 = squared(ROUTE_CLEARANCE_OF_ENEMY_MAIN_TILES * ts);
        if route.iter().any(|point| {
            dist2(point.0, point.1, objective.0, objective.1) < target_clearance2
                || dist2(point.0, point.1, enemy_main.0, enemy_main.1) < main_clearance2
        }) {
            return None;
        }
        Some(PincerSide {
            staging: stored(staging),
            attack: stored(attack),
            approach_from: stored(at(standoff_tiles
                + STAGING_EXTRA_TILES
                + APPROACH_EXTRA_TILES)),
        })
    };
    SIDE_ANGLES_DEGREES
        .iter()
        .find_map(|angle| Some([side(*angle)?, side(-*angle)?]))
}

fn cached_pincer_sides(
    memory: &mut AiDecisionMemory,
    analysis: &AiMapAnalysis,
    observation: &AiObservation,
    own_base: (f32, f32),
    enemy_main: (f32, f32),
    objective: (f32, f32),
    standoff_tiles: f32,
) -> Option<[PincerSide; 2]> {
    let key = stored(objective);
    if let Some(lanes) = &memory.pincer_lanes {
        if lanes.objective == key && observation.tick.saturating_sub(lanes.tick) < LANE_CACHE_TICKS
        {
            return lanes.sides;
        }
    }
    let sides = pincer_sides(
        analysis,
        observation,
        own_base,
        enemy_main,
        objective,
        standoff_tiles,
    );
    memory.pincer_lanes = Some(PincerLanes {
        objective: key,
        tick: observation.tick,
        sides,
    });
    sides
}

fn in_contact(push: &ContainmentPush, tick: u32) -> bool {
    push.contact_last_tick
        .is_some_and(|last| tick.saturating_sub(last) <= CONTAINMENT_CONTACT_MEMORY_TICKS)
}

/// In contact at the target itself, not on the way.
fn engaged_at_target(
    pincer: &Pincer,
    index: usize,
    push: &ContainmentPush,
    observation: &AiObservation,
) -> bool {
    if !in_contact(push, observation.tick) {
        return false;
    }
    let tanks: Vec<u32> = push.active_tanks.iter().copied().collect();
    let Some(center) = group_center(observation, &tanks) else {
        return false;
    };
    let objective = world(pincer.objective);
    let staging = world(pincer.sides[index].staging);
    let reach = dist2(staging.0, staging.1, objective.0, objective.1).sqrt()
        + ENGAGED_MARGIN_TILES * observation.map.tile_size as f32;
    dist2(center.0, center.1, objective.0, objective.1) <= squared(reach)
}

fn push_units(push: &ContainmentPush) -> Vec<u32> {
    push.active_tanks
        .iter()
        .copied()
        .chain(push.active_scout)
        .chain(push.active_riflemen.iter().copied())
        .collect()
}

/// This decision's orders for group `index`, taking a creep step when it is in contact and its
/// partner is not yet in place.
fn prong_orders(
    pincer: &mut Pincer,
    index: usize,
    push: &ContainmentPush,
    observation: &AiObservation,
) -> ProngOrders {
    let tick = observation.tick;
    let side = pincer.sides[index];
    let partner = 1 - index;
    let mut creep_step = false;
    if !pincer.advancing
        && !pincer.arrived[partner]
        && push.wave_launched
        && !push.recovery_active
        && engaged_at_target(pincer, index, push, observation)
        && pincer.last_creep_tick[index]
            .is_none_or(|last| tick.saturating_sub(last) >= CREEP_INTERVAL_TICKS)
    {
        let tanks: Vec<u32> = push.active_tanks.iter().copied().collect();
        let attack = world(side.attack);
        if let Some(center) = group_center(observation, &tanks) {
            if let Some(dir) = normalized_direction(center, attack) {
                let remaining = dist2(center.0, center.1, attack.0, attack.1).sqrt();
                let step = (CREEP_STEP_TILES * observation.map.tile_size as f32).min(remaining);
                pincer.hold[index] = stored((center.0 + dir.0 * step, center.1 + dir.1 * step));
                pincer.last_creep_tick[index] = Some(tick);
                creep_step = true;
            }
        }
    }
    let destination = if pincer.advancing {
        world(side.attack)
    } else {
        world(pincer.hold[index])
    };
    ProngOrders {
        destination,
        approach_from: world(side.approach_from),
        creep_step,
    }
}

/// Record whether group `index` is in place: at its waiting point, or already fighting there.
fn note_prong(
    pincer: &mut Pincer,
    index: usize,
    push: &ContainmentPush,
    observation: &AiObservation,
) {
    if pincer.arrived[index] || !push.wave_launched || push.recovery_active {
        return;
    }
    if push.at_destination || engaged_at_target(pincer, index, push, observation) {
        pincer.arrived[index] = true;
        pincer.first_arrival_tick.get_or_insert(observation.tick);
    }
}

/// End the pincer and send the partner's units back to the regroup point. They are free again
/// for the home reserve and the next push.
fn dissolve_partner(
    actions: &mut AiActionContext<'_>,
    memory: &mut AiDecisionMemory,
    rally: Option<(f32, f32)>,
) {
    let units = push_units(&memory.partner_push);
    if let Some(rally) = rally {
        actions::move_units(actions, units, rally.0, rally.1);
    }
    memory.partner_push = ContainmentPush::default();
    memory.pincer = None;
}

/// End the pincer by folding the partner into the main group, which carries on as one push.
fn merge_partner(memory: &mut AiDecisionMemory) {
    let partner = std::mem::take(&mut memory.partner_push);
    memory.containment.active_tanks.extend(partner.active_tanks);
    memory
        .containment
        .active_riflemen
        .extend(partner.active_riflemen);
    memory.containment.launch_tanks = memory.containment.active_tanks.len();
    memory.containment.march_waypoint = None;
    memory.containment.route.clear();
    memory.containment.route_index = 0;
    memory.containment.route_objective = None;
    memory.pincer = None;
}

#[allow(clippy::too_many_arguments)]
fn maybe_form_pincer(
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    memory: &mut AiDecisionMemory,
    map_analysis: Option<&AiMapAnalysis>,
    own_base: (f32, f32),
    enemy_base: EnemyBaseFact,
    objective: (f32, f32),
    standoff_tiles: f32,
    prong_min: usize,
) {
    if memory.pincer.is_some()
        || !memory.partner_push.active_tanks.is_empty()
        || memory.containment.active_tanks.is_empty()
        || memory.containment.recovery_active
        || memory.enemy_natural_destroyed
        || memory.enemy_main_destroyed
    {
        return;
    }
    let Some(analysis) = map_analysis else {
        return;
    };
    // Home first: no second group while the main is short of its reserve.
    if later_bases::main_tank_ids(observation, memory).len() < memory.home_tank_reserve() {
        return;
    }
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let is_kind = |id: &u32, kind: EntityKind| by_id.get(id).is_some_and(|unit| unit.kind == kind);
    let main_tanks: Vec<u32> = memory.containment.active_tanks.iter().copied().collect();
    let main_scout = memory.containment.active_scout;
    let launched = memory.containment.wave_launched;
    // Before it leaves, the push already holds every Tank beyond the home reserve: split it in
    // two. Once it is out, only Tanks beyond the reserve that were built since can go.
    let (mut pool, partner_size) = if launched {
        let extras: Vec<u32> = plan
            .ready_units
            .iter()
            .copied()
            .filter(|id| is_kind(id, EntityKind::Tank))
            .filter(|id| {
                !main_tanks.contains(id)
                    && Some(*id) != memory.home_defensive_tank
                    && !memory.later_bases.guards.contains(id)
            })
            .collect();
        let spare = extras
            .len()
            .saturating_sub(super::push_keep_home(observation, memory));
        (extras, spare)
    } else {
        (main_tanks.clone(), main_tanks.len() / 2)
    };
    if partner_size < prong_min {
        memory.pincer_scout_wanted = false;
        return;
    }
    let Some(sides) = cached_pincer_sides(
        memory,
        analysis,
        observation,
        own_base,
        (enemy_base.x, enemy_base.y),
        objective,
        standoff_tiles,
    ) else {
        memory.pincer_scout_wanted = false;
        return;
    };
    let scout = plan
        .ready_units
        .iter()
        .copied()
        .filter(|id| is_kind(id, EntityKind::ScoutCar) && main_scout != Some(*id))
        .min_by(|left, right| {
            let to_base = |id: &u32| {
                by_id.get(id).map_or(f32::MAX, |unit| {
                    dist2(unit.x, unit.y, own_base.0, own_base.1)
                })
            };
            to_base(left)
                .total_cmp(&to_base(right))
                .then_with(|| left.cmp(right))
        });
    let Some(scout) = scout else {
        // Everything else is in place: get a second Scout Car out.
        memory.pincer_scout_wanted = true;
        return;
    };
    memory.pincer_scout_wanted = false;

    // The main group takes the side nearer to where it is now.
    let main_at = group_center(observation, &main_tanks).unwrap_or(own_base);
    let near = |side: &PincerSide| {
        let staging = world(side.staging);
        dist2(main_at.0, main_at.1, staging.0, staging.1)
    };
    let (main_side, partner_side) = if near(&sides[0]) <= near(&sides[1]) {
        (sides[0], sides[1])
    } else {
        (sides[1], sides[0])
    };

    select_nearest_units(
        observation,
        &mut pool,
        world(partner_side.staging),
        partner_size,
    );
    let partner_tanks: BTreeSet<u32> = pool.into_iter().collect();
    memory
        .containment
        .active_tanks
        .retain(|id| !partner_tanks.contains(id));
    let main_riflemen = memory.containment.active_riflemen.clone();
    let rally =
        containment_regroup_point(own_base, enemy_base, observation.map).unwrap_or(own_base);
    let mut escorts = select_rifle_escorts(observation, memory, rally);
    escorts.retain(|id| !main_riflemen.contains(id));
    memory.partner_push = ContainmentPush {
        active_tanks: partner_tanks,
        active_scout: Some(scout),
        active_riflemen: escorts.into_iter().collect(),
        assembly_started_tick: Some(observation.tick),
        ..ContainmentPush::default()
    };
    // A main group already out re-routes to its own side.
    if launched {
        memory.containment.march_waypoint = None;
        memory.containment.route.clear();
        memory.containment.route_index = 0;
        memory.containment.route_objective = None;
    }
    memory.pincer = Some(Pincer {
        objective: stored(objective),
        sides: [main_side, partner_side],
        hold: [main_side.staging, partner_side.staging],
        arrived: [false, false],
        first_arrival_tick: None,
        advancing: false,
        last_creep_tick: [None, None],
        closest: [None, None],
        last_progress_tick: [observation.tick, observation.tick],
    });
}

/// Drive the current Jeff's push, and its partner group when the push is two-pronged.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_pushes(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    policy: ExpansionContainmentPolicy,
    profile: &AiProfile,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    prong_min: usize,
) -> Option<AiIntent> {
    let tick = observation.tick;
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    let rally = containment_regroup_point(own_base, enemy_base, observation.map);
    let objective = enemy_natural_edge(observation, enemy_base);

    // The partner falls back on its own losses like the main group does.
    if memory.pincer.is_some() {
        std::mem::swap(&mut memory.containment, &mut memory.partner_push);
        sync_containment_recovery(observation, profile, memory);
        std::mem::swap(&mut memory.containment, &mut memory.partner_push);
    }
    if let Some(pincer) = &memory.pincer {
        let partner = &memory.partner_push;
        let partner_failed = partner.recovery_active
            || partner.active_tanks.is_empty()
            || (partner.wave_launched && partner.active_scout.is_none());
        let main_failed =
            memory.containment.recovery_active || memory.containment.active_tanks.is_empty();
        let target_moved = memory.enemy_natural_destroyed
            || memory.enemy_main_destroyed
            || objective.is_none_or(|objective| {
                let pinned = world(pincer.objective);
                dist2(objective.0, objective.1, pinned.0, pinned.1)
                    > squared(OBJECTIVE_SHIFT_TILES * observation.map.tile_size as f32)
            });
        if partner_failed || main_failed {
            dissolve_partner(actions, memory, rally);
        } else if target_moved {
            merge_partner(memory);
        }
    }
    if let Some(objective) = objective {
        maybe_form_pincer(
            observation,
            plan,
            memory,
            map_analysis,
            own_base,
            enemy_base,
            objective,
            policy.tank_standoff_tiles,
            prong_min,
        );
    }

    // Main group.
    let mut pincer = memory.pincer.take();
    let main_orders = pincer
        .as_mut()
        .map(|pincer| prong_orders(pincer, 0, &memory.containment, observation));
    memory.pincer = pincer;
    let start = actions.emitted_len();
    let main_intent = issue_expansion_containment_wave(
        actions,
        observation,
        plan,
        enemy_base,
        policy,
        true,
        true,
        true,
        main_orders,
        map_analysis,
        memory,
    );
    note_containment_holds(actions, memory, start);

    // Partner group, through the same code with its state swapped in.
    let mut partner_intent = None;
    if memory.pincer.is_some() {
        std::mem::swap(&mut memory.containment, &mut memory.partner_push);
        let mut pincer = memory.pincer.take();
        let orders = pincer
            .as_mut()
            .map(|pincer| prong_orders(pincer, 1, &memory.containment, observation));
        memory.pincer = pincer;
        let start = actions.emitted_len();
        partner_intent = issue_expansion_containment_wave(
            actions,
            observation,
            plan,
            enemy_base,
            policy,
            true,
            true,
            true,
            orders,
            map_analysis,
            memory,
        );
        note_containment_holds(actions, memory, start);
        std::mem::swap(&mut memory.containment, &mut memory.partner_push);
    }

    // Timing: both in place and they go in together. A late group that keeps closing in is waited
    // for; one that stalls far away or runs out the clock calls the pincer off.
    let mut pincer = memory.pincer.take();
    if let Some(p) = pincer.as_mut() {
        note_prong(p, 0, &memory.containment, observation);
        note_prong(p, 1, &memory.partner_push, observation);
        let distance_to_hold = |p: &Pincer, index: usize, push: &ContainmentPush| {
            let tanks: Vec<u32> = push.active_tanks.iter().copied().collect();
            let hold = world(p.hold[index]);
            group_center(observation, &tanks).map(|center| {
                dist2(center.0, center.1, hold.0, hold.1).sqrt()
                    / observation.map.tile_size.max(1) as f32
            })
        };
        let distances = [
            distance_to_hold(p, 0, &memory.containment),
            distance_to_hold(p, 1, &memory.partner_push),
        ];
        for (index, distance) in distances.iter().enumerate() {
            if let Some(distance) = distance {
                let improved = p.closest[index]
                    .is_none_or(|closest| *distance + PROGRESS_TILES <= closest as f32);
                if improved {
                    p.closest[index] = Some(distance.round() as i32);
                    p.last_progress_tick[index] = tick;
                }
            }
        }
        if !p.advancing {
            if p.arrived == [true, true] {
                p.advancing = true;
            } else if let Some(first) = p.first_arrival_tick {
                let late = if p.arrived[0] { 1 } else { 0 };
                let stalled = tick.saturating_sub(p.last_progress_tick[late]) >= STALL_TICKS;
                let out_of_time = tick.saturating_sub(first) >= SYNC_LIMIT_TICKS;
                if stalled || out_of_time {
                    let near = distances[late].is_some_and(|distance| distance <= NEAR_TILES);
                    let late_set_out = if late == 0 {
                        memory.containment.wave_launched
                    } else {
                        memory.partner_push.wave_launched
                    };
                    if near && late_set_out {
                        p.advancing = true;
                    } else {
                        // Too far to arrive together: the group in place carries on as one push.
                        memory.pincer = None;
                        if late == 1 {
                            dissolve_partner(actions, memory, rally);
                        } else {
                            std::mem::swap(&mut memory.containment, &mut memory.partner_push);
                            dissolve_partner(actions, memory, rally);
                        }
                        return main_intent.or(partner_intent);
                    }
                }
            }
        }
    }
    memory.pincer = pincer;
    main_intent.or(partner_intent)
}

#[cfg(test)]
#[path = "pincer_tests.rs"]
mod tests;
