use super::*;
use crate::ai_core::profiles::JEFFS_AI;

fn map_observation(name: &str, player: u32) -> (AiObservation, AiMapAnalysis) {
    use rts_sim::game::map::Map;
    use rts_sim::game::{Game, PlayerInit};
    let players: Vec<_> = (1..=2)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".into(),
            name: format!("P{id}"),
            color: "#ffffff".into(),
            is_ai: true,
        })
        .collect();
    let map = Map::load_for_players(name, &[(1, 1), (2, 2)], 0x1234_5678).unwrap();
    let game = Game::new_with_random_ai_profiles_and_map_metadata(
        &players,
        0x1234_5678,
        map,
        Map::metadata_for_name(name).unwrap(),
    );
    let start = game.start_payload();
    let observation = AiObservation::from_snapshot_with_alive(
        &start,
        &game.snapshot_for(player),
        player,
        [],
        None,
    )
    .unwrap();
    (observation, AiMapAnalysis::analyze(&start))
}

fn lanes(name: &str, player: u32) -> Option<[PincerSide; 2]> {
    let (observation, analysis) = map_observation(name, player);
    let facts = AiFacts::from_observation(&observation);
    let enemy_base = facts.nearest_public_enemy_base.unwrap();
    let objective = enemy_natural_edge(&observation, enemy_base).unwrap();
    let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
    pincer_sides(
        &analysis,
        &observation,
        own_base,
        (enemy_base.x, enemy_base.y),
        objective,
        JEFFS_AI.expansion_containment.unwrap().tank_standoff_tiles,
    )
}

#[test]
fn every_map_offers_two_sides_to_the_enemy_natural() {
    for name in ["Classic", "The River", "Schone Tage", "Crossroads"] {
        for player in [1, 2] {
            let (observation, _) = map_observation(name, player);
            let facts = AiFacts::from_observation(&observation);
            let enemy_base = facts.nearest_public_enemy_base.unwrap();
            let objective = enemy_natural_edge(&observation, enemy_base).unwrap();
            let sides = lanes(name, player).unwrap_or_else(|| panic!("{name} p{player}"));
            for side in sides {
                let from_target = |point: WorldPoint| {
                    let point = world(point);
                    dist2(point.0, point.1, objective.0, objective.1).sqrt()
                };
                assert!(
                    from_target(side.staging) > from_target(side.attack),
                    "{name} p{player}"
                );
            }
            // The two sides are well apart around the target, not one lane twice.
            let dir = |point: WorldPoint| normalized_direction(objective, world(point)).unwrap();
            let (left, right) = (dir(sides[0].attack), dir(sides[1].attack));
            assert!(
                left.0 * right.0 + left.1 * right.1 < 0.6,
                "{name} p{player}"
            );
        }
    }
}

/// A pincer on an open 64-tile test map, the target at (40, 40) and Jeff's base to the south-west.
fn test_pincer() -> Pincer {
    let side = |staging: (i32, i32), attack: (i32, i32)| PincerSide {
        staging: (staging.0 * 32, staging.1 * 32),
        attack: (attack.0 * 32, attack.1 * 32),
        approach_from: (staging.0 * 32, staging.1 * 32),
    };
    let sides = [side((20, 40), (28, 40)), side((40, 20), (40, 28))];
    Pincer {
        objective: (40 * 32, 40 * 32),
        sides,
        hold: [sides[0].staging, sides[1].staging],
        arrived: [false, false],
        first_arrival_tick: None,
        advancing: false,
        last_creep_tick: [None, None],
        closest: [None, None],
        last_progress_tick: [0, 0],
    }
}

fn launched_push(
    tanks_at: (f32, f32),
    contact_tick: Option<u32>,
) -> (AiObservation, ContainmentPush) {
    let owned: Vec<AiEntitySummary> = (1..=4)
        .map(|id| {
            let mut tank = AiEntitySummary {
                id,
                owner: 1,
                kind: EntityKind::Tank,
                x: (tanks_at.0 + id as f32 * 0.5) * 32.0,
                y: tanks_at.1 * 32.0,
                hp: 300,
                state: AiEntityState::Idle,
                is_complete: true,
                production_queue_len: None,
                production_kind: None,
                latched_node: None,
                target_id: None,
                free_for_combat: true,
            };
            tank.owner = 1;
            tank
        })
        .collect();
    let observation = AiObservation {
        player_id: 1,
        tick: 3000,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size: 32,
        },
        economy: crate::ai_core::observation::AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (4, 60),
        players: Vec::new(),
        owned,
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: Vec::new(),
        ability_states: Vec::new(),
        smokes: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };
    let push = ContainmentPush {
        active_tanks: (1..=4).collect(),
        active_scout: Some(99),
        wave_launched: true,
        launch_tanks: 4,
        contact_last_tick: contact_tick,
        ..ContainmentPush::default()
    };
    (observation, push)
}

#[test]
fn each_group_waits_at_its_own_side_until_both_are_in_place() {
    let mut pincer = test_pincer();
    let (observation, mut push) = launched_push((10.0, 40.0), None);
    let orders = prong_orders(&mut pincer, 0, &push, &observation);
    assert_eq!(
        orders.destination,
        (20.0 * 32.0, 40.0 * 32.0),
        "its staging point"
    );
    assert!(!orders.creep_step);

    push.at_destination = true;
    note_prong(&mut pincer, 0, &push, &observation);
    assert_eq!(pincer.arrived, [true, false]);
    // Not in contact: it simply waits at its staging point for the partner.
    let orders = prong_orders(&mut pincer, 0, &push, &observation);
    assert_eq!(orders.destination, (20.0 * 32.0, 40.0 * 32.0));
    assert!(!orders.creep_step);
}

#[test]
fn a_group_in_contact_creeps_forward_until_its_partner_arrives() {
    let mut pincer = test_pincer();
    let (mut observation, push) = launched_push((20.0, 40.0), Some(3000));
    let first = prong_orders(&mut pincer, 0, &push, &observation);
    assert!(first.creep_step, "contact with the partner still away");
    assert!(
        first.destination.0 > 20.0 * 32.0,
        "a short step toward the target"
    );
    assert!(first.destination.0 < 25.0 * 32.0);

    // The next step waits out the interval.
    observation.tick += 60;
    let push_now = ContainmentPush {
        contact_last_tick: Some(observation.tick),
        ..push.clone()
    };
    assert!(!prong_orders(&mut pincer, 0, &push_now, &observation).creep_step);
    observation.tick += CREEP_INTERVAL_TICKS;
    let push_now = ContainmentPush {
        contact_last_tick: Some(observation.tick),
        ..push.clone()
    };
    assert!(prong_orders(&mut pincer, 0, &push_now, &observation).creep_step);

    // Once the partner is in place, both go in to their attack points: no more creeping.
    pincer.arrived[1] = true;
    pincer.advancing = true;
    observation.tick += CREEP_INTERVAL_TICKS;
    let orders = prong_orders(&mut pincer, 0, &push_now, &observation);
    assert!(!orders.creep_step);
    assert_eq!(orders.destination, (28.0 * 32.0, 40.0 * 32.0));
}
