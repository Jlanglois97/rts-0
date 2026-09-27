use std::collections::HashMap;

use crate::game::entity::EntityStore;
use crate::game::entrenchment_combat;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::teams::TeamRelations;
use crate::protocol::Event;
use crate::rules::combat as combat_rules;
use crate::rules::projection as projection_rules;
use crate::rules::terrain::TerrainKind;
use rand::Rng;

use super::events::{
    attack_reveal_for, emit_attack_event, emit_miss_event,
    push_under_attack_notices_for_visible_attack,
};
use super::projection::resolve_shot_victim;
use super::shot_blocker_index::ShotBlockerIndex;

#[derive(Clone, Copy)]
pub(super) struct ShotOutcome {
    pub(super) victim_owner: u32,
    pub(super) reveals_attacker: bool,
}

/// Apply `dmg` to `victim` from `attacker`, emitting an `Attack` event for every fired shot.
/// Returns the resolved shot outcome when a shot was emitted. Death itself is
/// handled by the death system (we only zero hp here).
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_damage(
    map: &Map,
    entities: &mut EntityStore,
    blockers: &ShotBlockerIndex,
    teams: &TeamRelations,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    rng: &mut impl Rng,
    attacker: u32,
    victim: u32,
    weapon_profile: &combat_rules::WeaponProfile,
    dmg: u32,
    attacker_owner: u32,
    ax: f32,
    ay: f32,
    vx: f32,
    vy: f32,
    extra_miss_chance: f32,
    tick: u32,
) -> Option<ShotOutcome> {
    if entities
        .get(victim)
        .map(|e| !e.is_targetable())
        .unwrap_or(false)
    {
        return None;
    }
    let shot_victim = resolve_shot_victim(
        map,
        entities,
        blockers,
        teams,
        attacker,
        victim,
        attacker_owner,
        ax,
        ay,
    );
    let shot_victim = shot_victim?;
    let shot_victim_pos = entities
        .get(shot_victim)
        .map(|e| (e.pos_x, e.pos_y))
        .unwrap_or((vx, vy));
    let victim = entities.get(shot_victim);
    let victim_kind = victim.map(|e| e.kind);
    let reveals_attacker = victim_kind.is_some_and(projection_rules::shot_reveals_attacker);
    let reveal = reveals_attacker
        .then(|| attack_reveal_for(entities.get(attacker)))
        .flatten();
    let victim_facing = victim.map(|e| e.facing());
    let victim_owner = entities.get(shot_victim).map(|e| e.owner).unwrap_or(0);
    let attack_recipients = emit_attack_event(
        events,
        fog,
        teams,
        attacker,
        shot_victim,
        attacker_owner,
        ax,
        ay,
        shot_victim_pos.0,
        shot_victim_pos.1,
        reveal.clone(),
        Some(weapon_profile.id.stable_id()),
    );

    // Intended targets have no intrinsic weapon miss roll. Movement penalties may still miss.
    let primary_missed = if entities.get(shot_victim).is_some() {
        let mc = extra_miss_chance.clamp(0.0, 1.0);
        if mc > 0.0 && rng.gen::<f32>() < mc {
            emit_miss_event(events, &attack_recipients, shot_victim);
            true
        } else {
            false
        }
    } else {
        false
    };
    let unentrenched_dmg = match victim_kind {
        Some(vk) => combat_rules::effective_damage_with_facing_for_weapon(
            weapon_profile,
            vk,
            dmg,
            Some(TerrainKind::Open),
            victim_facing,
            shot_victim_pos,
            (ax, ay),
        ),
        _ => dmg,
    };
    let entrenched_dmg = entities
        .get(shot_victim)
        .map(|victim| entrenchment_combat::reduce_direct_damage(victim, unentrenched_dmg))
        .unwrap_or(unentrenched_dmg);
    let effective_dmg =
        map.damage_after_reduction_tile(shot_victim_pos.0, shot_victim_pos.1, entrenched_dmg);
    let damaged = if primary_missed {
        false
    } else if let Some(v) = entities.get_mut(shot_victim) {
        if teams.is_enemy_owner(attacker_owner, v.owner) {
            v.apply_damage_from_entity(effective_dmg, attacker_owner, attacker, (ax, ay), tick)
        } else {
            v.apply_damage(effective_dmg, None)
        }
    } else {
        false
    };
    if damaged {
        if teams.is_enemy_owner(attacker_owner, victim_owner)
            && combat_rules::weapon_triggers_tank_armor_reaction(weapon_profile)
        {
            if let Some(victim) = entities.get_mut(shot_victim) {
                victim.lock_tank_armor_reaction_source((ax, ay), tick);
            }
        }
        push_under_attack_notices_for_visible_attack(
            events,
            fog,
            teams,
            victim_owner,
            attacker_owner,
            ax,
            ay,
            shot_victim_pos.0,
            shot_victim_pos.1,
        );
    }
    victim_kind.map(|_| ShotOutcome {
        victim_owner,
        reveals_attacker,
    })
}
