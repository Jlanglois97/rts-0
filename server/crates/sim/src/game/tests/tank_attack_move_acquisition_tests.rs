use super::fixtures::*;
use super::*;
use crate::game::entity::MovePhase;

fn refresh_fixture(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);
}

#[test]
fn charged_attack_moving_tank_checks_range_before_starting_to_move() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let tank_pos = (100.0, 100.0);
    let enemy_pos = (356.0, 100.0); // Visible, beyond moving range, inside charged range.
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, tank_pos.0, tank_pos.1)
        .expect("tank should spawn");
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    let unit = game
        .state
        .entities
        .get_mut(tank)
        .expect("tank should exist");
    unit.combat
        .as_mut()
        .expect("tank has combat state")
        .tank_stationary_range_ticks = 90;
    unit.set_weapon_cooldown(combat::WeaponKind::TankCannon, 20);
    refresh_fixture(&mut game);
    assert!(game.state.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1));

    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![tank],
            x: 600.0,
            y: 100.0,
            queued: false,
        },
    );
    game.tick();

    let unit = game.state.entities.get(tank).expect("tank should survive");
    assert_eq!((unit.pos_x, unit.pos_y), tank_pos);
    assert_eq!(unit.target_id(), Some(enemy));
    assert!(
        unit.path_is_empty(),
        "tank should hold its attack-move path"
    );
    assert!(
        unit.combat
            .as_ref()
            .expect("tank has combat state")
            .tank_stationary_range_ticks
            >= 90,
        "tank should keep its charged range while waiting to fire"
    );
}

#[test]
fn paused_attack_move_reacquires_a_new_target_during_reload() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let tank = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let former = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, 130.0, 100.0)
        .expect("former target should spawn");
    let replacement = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Tank, 200.0, 100.0)
        .expect("replacement target should spawn");
    let unit = game
        .state
        .entities
        .get_mut(tank)
        .expect("tank should exist");
    unit.set_order(Order::attack_move_to(500.0, 100.0));
    unit.mark_move_phase(MovePhase::Moving);
    unit.set_path_goal(Some((500.0, 100.0)));
    unit.set_target_id(Some(former));
    unit.set_weapon_cooldown(combat::WeaponKind::TankCannon, 20);
    game.state.entities.remove(former);
    refresh_fixture(&mut game);

    game.tick();

    let unit = game.state.entities.get(tank).expect("tank should survive");
    assert_eq!(unit.target_id(), Some(replacement));
    assert!(unit.path_is_empty(), "tank should not resume its route");
    assert_eq!((unit.pos_x, unit.pos_y), (100.0, 100.0));
}
