use crate::game::entity::{EntityKind, EntityStore, Order};

pub(super) fn apply(entities: &mut EntityStore, player: u32, units: &[u32], x: f32, y: f32) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let mut tanks: Vec<_> = units
        .iter()
        .copied()
        .filter(|id| {
            entities
                .get(*id)
                .is_some_and(|e| e.owner == player && e.kind == EntityKind::Tank && e.hp > 0)
        })
        .collect();
    tanks.sort_unstable();
    if tanks.is_empty() {
        return;
    }
    let (sx, sy) = tanks
        .iter()
        .filter_map(|id| entities.get(*id))
        .fold((0.0_f64, 0.0_f64), |(x, y), e| {
            (x + e.pos_x as f64, y + e.pos_y as f64)
        });
    let dx = x as f64 - sx / tanks.len() as f64;
    let dy = y as f64 - sy / tanks.len() as f64;
    if dx.hypot(dy) < 0.001 {
        return;
    }
    let facing = dy.atan2(dx) as f32;
    for id in tanks {
        if let Some(tank) = entities.get_mut(id) {
            tank.hold_position();
            tank.set_order(Order::Point { facing });
        }
    }
}
