use super::*;

#[test]
fn compact_build_stage_carries_building_kind() {
    let marker = OrderPlanMarker {
        kind: "build".to_string(),
        x: 384.0,
        y: 352.0,
        building_kind: Some("training_centre".to_string()),
    };
    let value = serde_json::to_value(CompactOrderPlanMarker(&marker)).unwrap();
    assert_eq!(
        value,
        serde_json::json!([5, 384.0, 352.0, kind_code("training_centre")])
    );
}
