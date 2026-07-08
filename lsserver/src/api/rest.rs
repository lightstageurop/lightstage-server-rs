use std::{net::SocketAddr, sync::Arc};

use axum::{
    Json,
    extract::{FromRef, State},
};
use tokio::net::TcpListener;
use tracing::info;
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_rapidoc::RapiDoc;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    config::ServerConfig,
    state::{SharedState, StageMode, StageState},
};

#[derive(Clone)]
struct AppState {
    config: Arc<ServerConfig>,
    state: SharedState,
}

impl FromRef<AppState> for SharedState {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.state.clone()
    }
}

impl FromRef<AppState> for Arc<ServerConfig> {
    fn from_ref(app_state: &AppState) -> Self {
        app_state.config.clone()
    }
}

#[derive(OpenApi)]
// #[openapi()]
struct ApiDoc;

pub async fn start_server(config: ServerConfig, state: SharedState) {
    let app_state = AppState {
        config: Arc::new(config),
        state,
    };

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_config))
        .routes(routes!(get_mode, set_mode))
        .routes(routes!(set_manual_frame))
        .split_for_parts();

    // host swagger / rapidocs
    let app = router
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api))
        // the swagger router is already hosting the openapi spec so we can just do:
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/rapidoc"))
        // if we drop swagger we would otherwise do:
        // .merge(RapiDoc::with_url(
        //     "/rapidoc",
        //     "/apid-docs/openapi.json",
        //     api,
        // ))
        .with_state(app_state);

    // TODO make base path (ie. `/api/`) configurable

    // from config
    let addr = SocketAddr::new(config.api_ip, config.api_port);
    info!("Starting REST API. Listening on http://{addr}");

    // serve
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/api/config",
    responses(
        (status = 200, description = "Get config success", body = ServerConfig)
    )
)]
async fn get_config(State(config): State<Arc<ServerConfig>>) -> Json<ServerConfig> {
    Json(*config)
}

#[utoipa::path(
    get,
    path = "/api/mode",
    responses(
        (status = 200, description = "Get mode success", body = StageMode)
    )
)]
async fn get_mode(State(state): State<SharedState>) -> Json<StageMode> {
    let lock = state.read().unwrap();
    Json(lock.mode)
}

#[utoipa::path(
    post,
    path = "/api/mode",
    responses(
        (status = 200, description = "Set mode success")
    )
)]
async fn set_mode(State(state): State<SharedState>, Json(payload): Json<StageMode>) {
    let mut lock = state.write().unwrap();
    lock.mode = payload;
}

#[utoipa::path(
    post,
    path = "/api/manual/frame",
    responses(
        (status = 200, description = "Set manual frame success")
    )
)]
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
