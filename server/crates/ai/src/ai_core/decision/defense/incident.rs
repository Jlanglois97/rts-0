use super::*;

pub(super) const SEARCH_TICKS: u32 = config::TICK_HZ * 2;
const REACQUIRE_TILES: f32 = 1.5;
/// A Tank that has not moved for three seconds reaches this range (the simulation ramps it from
/// the 5-tile base). Any movement or hull turn resets it to the base range.
const STATIONARY_TANK_RANGE_TILES: f32 = 14.0;
/// A Tank beyond stationary range closes to here and parks, so it settles inside full range
/// before the attackers reach it.
const STATIONARY_TANK_CLOSE_TO_TILES: f32 = 12.0;
/// Spotters stop this far short of the threat: inside the 10-tile sight radius, outside the
/// Rifleman and Machine Gunner reach.
const SPOTTER_STANDOFF_TILES: f32 = 8.0;
const MAX_SPOTTERS: usize = 2;

/// `stationary_tanks` keeps defending Tanks parked for their stationary range bonus and sends
/// infantry forward to provide the vision for those long shots, instead of ordering the Tanks to
/// attack (which drives them to the 5-tile base range).
pub(in crate::ai_core::decision) fn respond_to_local_incident(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    local_defenders: &[u32],
    stationary_tanks: bool,
) -> Option<Vec<u32>> {
    if let Some(contact) = local_defense_contact(observation) {
        memory.note_defensive_contact(
            observation.tick,
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let mut interceptors = select_defensive_interceptors(
            observation,
            memory,
            eligible_local_defenders(observation, local_defenders),
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let smoke = crate::ai_core::decision::frontal::smoke::maybe_issue_local_defense_smoke(
            actions,
            observation,
            &interceptors,
            local_defenders,
            &contact.target_ids,
            memory,
        );
        let mut attack_targets = contact.target_ids.clone();
        if let Some(smoke) = smoke {
            let scout = match smoke {
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Obscure {
                    target,
                    scout,
                } => {
                    attack_targets.retain(|candidate| *candidate != target);
                    scout
                }
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Reposition {
                    scout,
                } => scout,
            };
            interceptors.retain(|unit| *unit != scout);
        }
        if stationary_tanks {
            if let Some(target) = primary_defense_target(observation, &attack_targets) {
                return stationary_tank_defense(
                    actions,
                    observation,
                    memory,
                    interceptors,
                    local_defenders,
                    &attack_targets,
                    target,
                );
            }
        }
        return if let Some(target) = primary_defense_target(observation, &attack_targets) {
            actions::attack_units(actions, interceptors, target)
        } else {
            actions::hold_position_units(actions, interceptors)
        };
    }

    let incident = memory.defensive_incident(observation.tick, SEARCH_TICKS)?;
    let candidates = local_defense_units_with_plans(observation, local_defenders);
    let reacquire2 = squared(REACQUIRE_TILES * observation.map.tile_size as f32);
    let reached_last_contact = candidates.iter().any(|id| {
        observation.owned.iter().any(|unit| {
            unit.id == *id
                && dist2(unit.x, unit.y, incident.position.0, incident.position.1) <= reacquire2
        })
    });
    if reached_last_contact {
        memory.clear_defensive_incident();
        memory.local_defense_held_tanks.clear();
        return None;
    }
    let mut interceptors = select_defensive_interceptors(
        observation,
        memory,
        candidates,
        incident.position,
        incident.threat_value,
        incident.armored_threat,
    );
    // Parked Tanks keep their range while infantry search; if nobody else can search, the Tanks
    // must go themselves.
    if stationary_tanks
        && interceptors
            .iter()
            .any(|id| !memory.local_defense_held_tanks.contains(id))
    {
        interceptors.retain(|id| !memory.local_defense_held_tanks.contains(id));
    } else {
        memory.local_defense_held_tanks.clear();
    }
    actions::attack_move_units(
        actions,
        interceptors,
        incident.position.0,
        incident.position.1,
    )
}

fn stationary_tank_defense(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    interceptors: Vec<u32>,
    local_defenders: &[u32],
    attack_targets: &[u32],
    target: u32,
) -> Option<Vec<u32>> {
    let ts = observation.map.tile_size as f32;
    let target_position = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == target)
        .map(|enemy| (enemy.x, enemy.y))?;
    let threats: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| attack_targets.contains(&enemy.id))
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    let owned = |id: u32| observation.owned.iter().find(|unit| unit.id == id);
    let (tanks, mut infantry): (Vec<u32>, Vec<u32>) = interceptors
        .into_iter()
        .partition(|id| owned(*id).is_some_and(|unit| unit.kind == EntityKind::Tank));
    memory
        .local_defense_held_tanks
        .retain(|id| tanks.contains(id));

    let mut assigned = Vec::new();
    let mut hold = Vec::new();
    for tank_id in &tanks {
        let Some(tank) = owned(*tank_id) else {
            continue;
        };
        let in_range = threats.iter().any(|threat| {
            dist2(tank.x, tank.y, threat.0, threat.1) <= squared(STATIONARY_TANK_RANGE_TILES * ts)
        });
        if in_range {
            // Holding clears the Tank's target, so re-issue it only when something moved it.
            if !memory.local_defense_held_tanks.contains(tank_id)
                || tank.state == AiEntityState::Move
            {
                hold.push(*tank_id);
                memory.local_defense_held_tanks.insert(*tank_id);
            }
        } else {
            memory.local_defense_held_tanks.remove(tank_id);
            let direction =
                normalized_direction(target_position, (tank.x, tank.y)).unwrap_or((0.0, 1.0));
            let park = clamp_to_map(
                (
                    target_position.0 + direction.0 * STATIONARY_TANK_CLOSE_TO_TILES * ts,
                    target_position.1 + direction.1 * STATIONARY_TANK_CLOSE_TO_TILES * ts,
                ),
                observation.map,
            );
            if let Some(units) = actions::move_units(actions, [*tank_id], park.0, park.1) {
                assigned.extend(units);
            }
        }
    }
    if let Some(units) = actions::hold_position_units(actions, hold) {
        assigned.extend(units);
    }
    // Held Tanks keep their slot in the response even on ticks where no new order is needed.
    assigned.extend(memory.local_defense_held_tanks.iter().copied());

    // Parked Tanks outrange their own 10-tile sight: without infantry ahead of them, the long
    // shots have no vision. Add spotters when the response selected only armor.
    let mut spotters = Vec::new();
    if infantry.is_empty() && !tanks.is_empty() {
        let mut candidates: Vec<&AiEntitySummary> =
            eligible_local_defenders(observation, local_defenders)
                .into_iter()
                .filter_map(owned)
                .filter(|unit| {
                    matches!(unit.kind, EntityKind::Rifleman | EntityKind::MachineGunner)
                })
                .filter(|unit| {
                    memory.estimated_entrenchment_ticks(observation, unit.id)
                        < rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
                })
                .collect();
        candidates.sort_by(|left, right| {
            dist2(left.x, left.y, target_position.0, target_position.1)
                .total_cmp(&dist2(
                    right.x,
                    right.y,
                    target_position.0,
                    target_position.1,
                ))
                .then_with(|| left.id.cmp(&right.id))
        });
        spotters.extend(candidates.iter().take(MAX_SPOTTERS).map(|unit| unit.id));
    }
    for spotter_id in &spotters {
        let Some(spotter) = owned(*spotter_id) else {
            continue;
        };
        let direction =
            normalized_direction(target_position, (spotter.x, spotter.y)).unwrap_or((0.0, 1.0));
        let point = clamp_to_map(
            (
                target_position.0 + direction.0 * SPOTTER_STANDOFF_TILES * ts,
                target_position.1 + direction.1 * SPOTTER_STANDOFF_TILES * ts,
            ),
            observation.map,
        );
        if let Some(units) = actions::move_units(actions, [*spotter_id], point.0, point.1) {
            assigned.extend(units);
        }
    }

    // Selected infantry advance into contact as before, which also lights the targets.
    infantry.retain(|id| !spotters.contains(id));
    if let Some(units) = actions::attack_units(actions, infantry, target) {
        assigned.extend(units);
    }
    assigned.sort_unstable();
    assigned.dedup();
    (!assigned.is_empty()).then_some(assigned)
}

fn eligible_local_defenders(observation: &AiObservation, local_defenders: &[u32]) -> Vec<u32> {
    local_defense_units_with_plans(observation, local_defenders)
        .into_iter()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id
                    && matches!(
                        unit.kind,
                        EntityKind::Rifleman
                            | EntityKind::MachineGunner
                            | EntityKind::ScoutCar
                            | EntityKind::Panzerfaust
                            | EntityKind::Tank
                    )
            })
        })
        .collect()
}

pub(in crate::ai_core::decision) fn select_defensive_interceptors(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    mut candidates: Vec<u32>,
    contact: (f32, f32),
    threat_value: u32,
    armored_threat: bool,
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    candidates.sort_by(|left, right| {
        let left_counter_rank = by_id.get(left).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let right_counter_rank = by_id.get(right).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let left_entrenchment = memory.estimated_entrenchment_ticks(observation, *left);
        let right_entrenchment = memory.estimated_entrenchment_ticks(observation, *right);
        let left_dist = by_id
            .get(left)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        let right_dist = by_id
            .get(right)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        left_counter_rank
            .cmp(&right_counter_rank)
            .then_with(|| left_entrenchment.cmp(&right_entrenchment))
            .then_with(|| left_dist.total_cmp(&right_dist))
            .then_with(|| left.cmp(right))
    });
    candidates.dedup();
    candidates.retain(|unit_id| {
        by_id.get(unit_id).is_some_and(|unit| {
            unit.kind != EntityKind::Rifleman
                || memory.estimated_entrenchment_ticks(observation, *unit_id)
                    < rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
        })
    });

    // A two-to-one observed-value response clears a small penetration without uprooting every
    // entrenched edge guard. Larger forces naturally exhaust the available home reserve.
    let required_value = threat_value.saturating_mul(2).max(1);
    let mut selected = Vec::new();
    let mut selected_value: u32 = 0;
    let mut has_anti_armor = false;
    let eligible_candidates = candidates.clone();
    for unit_id in candidates {
        let Some(unit) = by_id.get(&unit_id) else {
            continue;
        };
        selected.push(unit_id);
        has_anti_armor |= matches!(unit.kind, EntityKind::Tank | EntityKind::Panzerfaust);
        let (steel, oil) = rts_rules::economy::cost(unit.kind);
        selected_value = selected_value.saturating_add(steel.saturating_add(oil).max(1));
        if selected_value >= required_value {
            break;
        }
    }
    let selected_has_tank = selected.iter().any(|unit_id| {
        by_id
            .get(unit_id)
            .is_some_and(|unit| unit.kind == EntityKind::Tank)
    });
    let river_natural =
        crate::ai_core::decision::expansion::has_jeff_river_expansion_site(observation);
    if selected_has_tank && river_natural {
        for kind in [EntityKind::Tank, EntityKind::Rifleman] {
            for unit_id in eligible_candidates
                .iter()
                .copied()
                .filter(|unit_id| by_id.get(unit_id).is_some_and(|unit| unit.kind == kind))
            {
                if !selected.contains(&unit_id) {
                    selected.push(unit_id);
                }
                let selected_of_kind = selected
                    .iter()
                    .filter(|selected_id| {
                        by_id.get(selected_id).is_some_and(|unit| unit.kind == kind)
                    })
                    .count();
                if selected_of_kind >= 2 {
                    break;
                }
            }
        }
    }
    if selected_value < required_value && !has_anti_armor {
        Vec::new()
    } else {
        selected
    }
}

fn defensive_counter_rank(kind: EntityKind, armored_threat: bool) -> u8 {
    if armored_threat {
        match kind {
            EntityKind::Tank | EntityKind::Panzerfaust => 0,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar => 1,
            _ => 2,
        }
    } else {
        0
    }
}

fn primary_defense_target(observation: &AiObservation, target_ids: &[u32]) -> Option<u32> {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| target_ids.contains(&enemy.id))
        .max_by(|left, right| {
            let left_armored = matches!(left.kind, EntityKind::Tank | EntityKind::ScoutCar);
            let right_armored = matches!(right.kind, EntityKind::Tank | EntityKind::ScoutCar);
            left_armored
                .cmp(&right_armored)
                .then_with(|| unit_value(left.kind).cmp(&unit_value(right.kind)))
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|enemy| enemy.id)
}
