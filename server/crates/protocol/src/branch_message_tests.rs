use super::*;

#[test]
fn branch_from_tick_created_serializes_contract_shape() {
    let msg = ServerMessage::BranchFromTickCreated {
        branch_room: "__replay_branch__:00000001".to_string(),
        source_tick: 123,
        seats: vec![ReplayBranchSeat {
            player_id: 7,
            team_id: 7,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Player 7".to_string(),
            color: "#4878c8".to_string(),
            claimable: true,
        }],
    };
    let json = serde_json::to_value(msg).expect("branch message should serialize");

    assert_eq!(json["t"], "branchFromTickCreated");
    assert_eq!(json["branchRoom"], "__replay_branch__:00000001");
    assert_eq!(json["sourceTick"], 123);
    assert_eq!(json["seats"][0]["playerId"], 7);
    assert_eq!(json["seats"][0]["teamId"], 7);
    assert_eq!(json["seats"][0]["factionId"], DEFAULT_FACTION_ID);
    assert_eq!(json["seats"][0]["name"], "Player 7");
    assert_eq!(json["seats"][0]["color"], "#4878c8");
    assert_eq!(json["seats"][0]["claimable"], true);
}
