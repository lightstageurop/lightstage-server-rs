use std::net::{IpAddr, SocketAddr};

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    api::ApiState,
    state::{SharedState, StageMode},
};

#[derive(Deserialize)]
struct ModeRequest {
    mode: StageMode,
}

pub async fn start_server(addr: IpAddr, port: u16, state: SharedState) {
    let api_state = ApiState { state };

    let app = Router::new()
        .route("/api/mode", post(set_mode))
        .route("/api/mode", get(get_mode))
        .route("/api/manual/frame", post(set_manual_frame))
        .with_state(api_state);
    // TODO make base path (ie. `/api/`) configurable

    let addr = SocketAddr::new(addr, port);
    info!("Starting REST API. Listening on http://{addr}");

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// GET `/api/mode`
async fn get_mode(State(api): State<ApiState>) -> Json<StageMode> {
    Json(api.get_mode())
}

/// POST `/api/mode`
async fn set_mode(State(api): State<ApiState>, Json(payload): Json<ModeRequest>) {
    api.set_mode(payload.mode);
}

/// POST `/api/manual/frame`
async fn set_manual_frame(
    State(state): State<SharedState>,
    Json(frame): Json<[[[u16; 6]; 14]; 12]>, // TODO is this really a good idea?
                                             // also this should probably follow num_arcs, lights_per_arc in config
) {
    let mut lock = state.write().unwrap();

    // enable manual mode automatically
    lock.mode = StageMode::Manual;

    for (arc_idx, arc_data) in frame.iter().enumerate() {
        for (light_idx, channels) in arc_data.iter().enumerate() {
            lock.renderer.rgb_fixtures[arc_idx][light_idx].set_color(
                channels[0],
                channels[1],
                channels[2],
            );
            lock.renderer.white_fixtures[arc_idx][light_idx].set_white(
                channels[3],
                channels[4],
                channels[5],
            );
        }
    }

    // TODO this should probably be a `update_and_render` method on StageState instead of doing this everywhere.
    let StageState {
        renderer,
        current_frame,
        ..
    } = &mut *lock;
    renderer.update(current_frame);
}
