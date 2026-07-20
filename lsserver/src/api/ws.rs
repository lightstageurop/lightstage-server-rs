//! # WebSocket API
//!
//! This interface provides much higher throughput and some more features over it's
//! REST counterpart. ([`crate::api::rest`]).
//! It should usually be preferred.
//!
//! The WebSocket endpoint is `/ws`.
//!
//! All messages are encoding using [CBOR][cbor].
//!
//! For a list of supported commands, see [`WsCommand`].
//! There is no further API documentation yet, as there is with REST.
//!
//! [cbor]: https://cbor.io/

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

use crate::{
    api::{ApiState, ModeRequest, UpdateColourRequest, UpdateFixturesRequest},
    config::ServerConfig,
    state::StageMode,
};

/// Inbound websocket commands
#[derive(Debug, Deserialize)]
pub enum WsCommand {
    /// Get the server's configuration.
    GetConfig,
    /// Get the current operation mode of the light stage.
    GetMode,
    /// Set the operation mode of the light stage.
    SetMode(ModeRequest),
    /// Set the entire light stage to a uniform colour.
    SetLightstage(UpdateColourRequest),
    ///  Set an arc to a uniform colour.
    SetArc {
        arc_idx: usize,
        colour: UpdateColourRequest,
    },
    /// Set a specific light to a colour.
    SetFixture {
        arc_idx: usize,
        light_idx: usize,
        colour: UpdateColourRequest,
    },
    /// Update multiple fixtures' colours.
    SetFixtures(Vec<UpdateFixturesRequest>),
    ManualTrigger,
}

/// Outgoing websocket response
#[derive(Debug, Clone, Serialize)]
pub enum WsResponse {
    /// The light stage's current operation mode
    Mode(StageMode),
    /// The server's config
    Config(ServerConfig),
    /// An error.
    Error { code: WsErrorKind, message: String },
}

/// Error codes that can be returned
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WsErrorKind {
    InvalidPayload,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(api): State<ApiState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, api))
}

async fn handle_socket(mut socket: WebSocket, api: ApiState) {
    debug!("Websocket client connected.");

    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Binary(bytes) => match ciborium::from_reader::<WsCommand, _>(&bytes[..]) {
                Ok(cmd) => {
                    if let Some(response) = execute_command(cmd, &api) {
                        if send_response(&mut socket, &response).await.is_err() {
                            error!("Failed to transmit websocket response! Dropping connection.");
                            break;
                        }
                    }
                }
                Err(e) => {
                    let err_response = WsResponse::Error {
                        code: WsErrorKind::InvalidPayload,
                        message: format!("Invalid CBOR payload: {e}"),
                    };
                    if send_response(&mut socket, &err_response).await.is_err() {
                        error!("Failed to transmit websocket error response! Dropping connection.");
                        break;
                    }
                }
            },
            Message::Close(_) => {
                debug!("Websocket client disconnected.");
                break;
            }
            _ => {} // not a binary message, ignore.
        }
    }
}

/// Serialise outbound respones into CBOR message and send.
async fn send_response(socket: &mut WebSocket, response: &WsResponse) -> anyhow::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    ciborium::into_writer(&response, &mut buf)?;
    socket.send(Message::Binary(buf.into())).await?;
    Ok(())
}

/// Interpret commands and update underlying state ([`ApiState`]).
fn execute_command(command: WsCommand, api: &ApiState) -> Option<WsResponse> {
    match command {
        WsCommand::GetConfig => Some(WsResponse::Config(api.config)),
        WsCommand::GetMode => Some(WsResponse::Mode(api.get_mode())),
        WsCommand::SetMode(mode) => match api.set_mode(mode) {
            Ok(()) => None,
            Err(err) => Some(WsResponse::Error {
                code: WsErrorKind::InvalidPayload,
                message: err.to_string(),
            }),
        },
        WsCommand::SetFixture {
            arc_idx,
            light_idx,
            colour,
        } => {
            api.set_fixture(arc_idx, light_idx, colour.rgb, colour.white);
            None
        }
        WsCommand::SetFixtures(fixtures) => {
            let mapped = fixtures
                .into_iter()
                .map(|req| (req.arc_idx, req.light_idx, req.colour.rgb, req.colour.white))
                .collect();
            api.set_fixtures(mapped);
            None
        }
        WsCommand::SetArc { arc_idx, colour } => {
            api.set_arc(arc_idx, colour.rgb, colour.white);
            None
        }
        WsCommand::SetLightstage(colour) => {
            api.set_lightstage(colour.rgb, colour.white);
            None
        }
        WsCommand::ManualTrigger => match api.trigger_manual() {
            Ok(()) => None,
            Err(err) => Some(WsResponse::Error {
                code: WsErrorKind::InvalidPayload,
                message: err.to_string(),
            }),
        },
    }
}
