use super::*;

fn mortar_launch_count(events: &HashMap<u32, Vec<Event>>, player: u32) -> usize {
    events
        .get(&player)
        .map(|player_events| {
            player_events
                .iter()
                .filter(|event| matches!(event, Event::MortarLaunch { .. }))
                .count()
        })
        .unwrap_or(0)
}

fn ready_autocast_mortar(entities: &mut EntityStore, id: u32) {
    if let Some(mortar) = entities.get_mut(id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }
}

#[test]
fn mortar_autocast_does_not_overcommit_lethal_fire() {
    let mut entities = EntityStore::new();
    let mortar_a = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("first mortar should spawn");
    let mortar_b = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 112.0)
        .expect("second mortar should spawn");
    let mortar_c = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 124.0)
        .expect("third mortar should spawn");
    let enemy_a = entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("first target should spawn");
    let enemy_b = entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 156.0)
        .expect("second target should spawn");
    for target_id in [enemy_a, enemy_b] {
        entities
            .get_mut(target_id)
            .expect("target should exist")
            .hp = config::MORTAR_OUTER_DAMAGE;
    }
    for mortar_id in [mortar_a, mortar_b, mortar_c] {
        ready_autocast_mortar(&mut entities, mortar_id);
    }

    let events = run_combat_tick(&mut entities);

    assert_eq!(
        mortar_launch_count(&events, 1),
        2,
        "only enough autocast mortars to kill the available targets should fire"
    );
    let mortar_a = entities.get(mortar_a).expect("first mortar should exist");
    let mortar_b = entities.get(mortar_b).expect("second mortar should exist");
    let mortar_c = entities.get(mortar_c).expect("third mortar should exist");
    assert_eq!(
        mortar_a.target_id(),
        Some(enemy_a),
        "first mortar should take the nearest lethal target"
    );
    assert_eq!(
        mortar_b.target_id(),
        Some(enemy_b),
        "second mortar should skip the already-covered target and fire at the next one"
    );
    assert_eq!(
        mortar_c.attack_cd(),
        0,
        "third mortar should keep its shot when every visible target already has lethal fire committed"
    );
}

#[test]
fn mortar_autocast_prefers_safe_target_over_nearer_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    let safe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        Some(safe_enemy),
        "autocast mortar should choose the best target with a clear predicted impact"
    );
    assert!(
        mortar.attack_cd() > 0,
        "autocast mortar should fire after switching to a safe target"
    );
}

#[test]
fn mortar_autocast_tracks_safe_target_while_reload_blocks_firing() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    let safe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 20.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_attack_cd(12);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar_entity = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar_entity.target_id(),
        Some(safe_enemy),
        "reloading autocast mortar should keep tracking the safe target"
    );
    let expected_turn = -mortar::TURN_RATE_RAD_PER_TICK;
    assert!(
        angle_delta(mortar_entity.facing(), expected_turn).abs() <= 0.001,
        "mortar should turn toward the safe target while reloading, got {:.4}",
        mortar_entity.facing()
    );
    assert!(
        mortar_entity.attack_cd() > 0,
        "test setup should keep the mortar unable to fire this tick"
    );
}

#[test]
fn mortar_autocast_drops_unsafe_target_when_no_safe_target_exists() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_target_id(Some(unsafe_enemy));
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        None,
        "autocast mortar should not keep an unsafe target when no safe target exists"
    );
    assert_eq!(
        mortar.attack_cd(),
        0,
        "autocast mortar should still hold fire when every candidate would splash same-team entities"
    );
}

#[test]
fn mortar_autocast_explicit_attack_keeps_commanded_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_order(Order::attack(unsafe_enemy));
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        Some(unsafe_enemy),
        "explicit attack intent should keep the commanded target"
    );
    assert_eq!(
        mortar.attack_cd(),
        0,
        "explicit attack should still hold fire when the commanded target would splash friendlies"
    );
}
