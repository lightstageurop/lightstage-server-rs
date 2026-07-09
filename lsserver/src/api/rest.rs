use std::net::SocketAddr;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tracing::info;
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_rapidoc::RapiDoc;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    api::{ApiState, FixtureColour},
    config::ServerConfig,
    state::{SharedState, StageMode},
};

const CONFIG_TAG: &str = "Configuration";
const MANUAL_TAG: &str = "Manual Control";

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateColourRequest {
    rgb: FixtureColour,
    white: FixtureColour,
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateFixturesRequest {
    arc_idx: usize,
    light_idx: usize,
    colour: UpdateColourRequest,
}

#[derive(OpenApi)]
#[openapi(info(title = "Light Stage API", description = include_str!("README.md")))]
struct ApiDoc;

pub async fn start_server(config: ServerConfig, state: SharedState) {
    let api_state = ApiState { state, config };

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_config))
        .routes(routes!(get_mode, set_mode))
        .routes(routes!(set_lightstage))
        .routes(routes!(set_arc))
        .routes(routes!(set_fixture))
        .routes(routes!(set_fixtures))
        .split_for_parts();

    // host swagger / rapidocs
    let app = router
        .merge(SwaggerUi::new("/api-docs/swagger-ui").url("/api-docs/openapi.json", api))
        // the swagger router is already hosting the openapi spec so we can just do:
        .merge(RapiDoc::new("/api-docs/openapi.json").path("/api-docs/rapidoc"))
        // if we drop swagger we would otherwise do:
        // .merge(RapiDoc::with_url(
        //     "/rapidoc",
        //     "/apid-docs/openapi.json",
        //     api,
        // ))
        .with_state(api_state);
    // TODO make base path (ie. `/api/`) configurable

    // from config
    let addr = SocketAddr::new(config.api_ip, config.api_port);
    info!("Starting REST API. Listening on http://{addr}");

    // serve
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// Get the server's configuration
#[utoipa::path(
    get,
    path = "/api/config",
    tag = CONFIG_TAG,
    responses(
        (status = 200, description = "Get config success", body = ServerConfig)
    )
)]
async fn get_config(State(api): State<ApiState>) -> Json<ServerConfig> {
    Json(api.config)
}

/// Get the current operation mode of the light stage.
#[utoipa::path(
    get,
    path = "/api/mode",
    tag = CONFIG_TAG,
    responses(
        (status = 200, description = "Get mode success", body = StageMode)
    )
)]
async fn get_mode(State(api): State<ApiState>) -> Json<StageMode> {
    Json(api.get_mode())
}

/// Set the operation mode of the light stage.
#[utoipa::path(
    post,
    path = "/api/mode",
    tag = CONFIG_TAG,
    responses(
        (status = 200, description = "Set mode success")
    )
)]
async fn set_mode(State(api): State<ApiState>, Json(payload): Json<StageMode>) {
    api.set_mode(payload);
}

/// Set the entire light stage to a uniform colour.
#[utoipa::path(
    put,
    path = "/api/manual/all",
    tag = MANUAL_TAG,
    responses(
        (status = 200, description = "Set entire lightstage success")
    )
)]
async fn set_lightstage(State(api): State<ApiState>, Json(payload): Json<UpdateColourRequest>) {
    api.set_lightstage(payload.rgb, payload.white);
}

/// Set an arc to a uniform colour.
#[utoipa::path(
    put,
    path = "/api/manual/arcs/{arc_idx}",
    tag = MANUAL_TAG,
    params(
        ("arc_idx" = u8, Path, description = "arc id")
    ),
    responses(
        (status = 200, description = "Set arc success")
    )
)]
async fn set_arc(
    State(api): State<ApiState>,
    Path(arc_idx): Path<u8>,
    Json(payload): Json<UpdateColourRequest>,
) {
    api.set_arc(arc_idx as usize, payload.rgb, payload.white);
}

/// Set a specific light to a colour.
#[utoipa::path(
    put,
    path = "/api/manual/arcs/{arc_idx}/light/{light_idx}",
    tag = MANUAL_TAG,
    params(
        ("arc_idx" = u8, Path, description = "arc id"),
        ("light_idx" = u8, Path)
    ),
    responses(
        (status = 200, description = "Set fixture success")
    )
)]
async fn set_fixture(
    State(api): State<ApiState>,
    Path((arc_idx, light_idx)): Path<(u8, u8)>,
    Json(payload): Json<UpdateColourRequest>,
) {
    api.set_fixture(
        arc_idx as usize,
        light_idx as usize,
        payload.rgb,
        payload.white,
    );
}

/// Update multiple fixtures' colours.
#[utoipa::path(
    patch,
    path = "/api/manual/fixtures",
    tag = MANUAL_TAG,
    responses(
        (status = 200, description = "Set fixtures success")
    )
)]
async fn set_fixtures(
    State(api): State<ApiState>,
    Json(payload): Json<Vec<UpdateFixturesRequest>>,
) {
    api.set_fixtures(
        payload
            .iter()
            .map(|req| (req.arc_idx, req.light_idx, req.colour.rgb, req.colour.white))
            .collect(),
    );
}
