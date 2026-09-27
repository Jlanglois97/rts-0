//! Match-history replay HTTP handlers: spectator replay launch and raw artifact download.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rts_sim::game::replay::ReplayArtifactV1;

use crate::{
    replay_incompatibility_reason, request_allows_local_match_history, ApiError, AppState,
    MatchReplayLaunchResponse,
};

/// Load a persisted replay artifact visible to this request, or the HTTP error response to return.
async fn load_visible_replay_artifact(
    state: &AppState,
    remote: &SocketAddr,
    match_id: i64,
) -> Result<ReplayArtifactV1, Response> {
    let Some(db) = state.db.clone() else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "Replay is unavailable because match history is not configured.".to_string(),
            }),
        )
            .into_response());
    };
    let include_local = request_allows_local_match_history(remote);
    match db.replay_artifact_for_match(match_id, include_local).await {
        Ok(Some(artifact)) => Ok(artifact),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                error: "Replay is unavailable for this match.".to_string(),
            }),
        )
            .into_response()),
        Err(err) => {
            rts_server::log_warn!(%err, match_id, "match replay load failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    error: "Replay could not be loaded.".to_string(),
                }),
            )
                .into_response())
        }
    }
}

/// GET /api/matches/{id}/replay-artifact — download the stored replay artifact for offline
/// analysis. Uses the same visibility scope as replay launch and skips build compatibility
/// checks, because offline tools re-simulate on the artifact's recorded build.
pub(crate) async fn match_replay_artifact_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(match_id): Path<i64>,
) -> impl IntoResponse {
    match load_visible_replay_artifact(&state, &remote, match_id).await {
        Ok(artifact) => (
            [(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"replay-{match_id}.json\""),
            )],
            Json(artifact),
        )
            .into_response(),
        Err(response) => response,
    }
}

/// POST /api/matches/{id}/replay — create a spectator replay room for a compatible persisted match.
pub(crate) async fn match_replay_launch_handler(
    State(state): State<AppState>,
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    Path(match_id): Path<i64>,
) -> impl IntoResponse {
    let artifact = match load_visible_replay_artifact(&state, &remote, match_id).await {
        Ok(artifact) => artifact,
        Err(response) => return response,
    };

    if let Some(reason) = replay_incompatibility_reason(&artifact, &state.version) {
        return (StatusCode::CONFLICT, Json(ApiError { error: reason })).into_response();
    }

    let room = state.lobby.persisted_replay_room(match_id, artifact).await;
    Json(MatchReplayLaunchResponse { room }).into_response()
}
