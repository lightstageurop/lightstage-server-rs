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
    state::{StageEvent, StageMode,
};

/// Inbound websocket request
#[derive(Debug, Clone, Deserialize)]
pub struct WsRequest {
    /// Optional command id, will be echoed by the server
    pub id: Option<u64>,
    pub command: WsCommand,
}

/// Inbound websocket commands
#[derive(Debug, Clone, Deserialize)]
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

/// Outbound websocket message
#[derive(Debug, Clone, Serialize)]
pub enum WsServerMessage {
    Response {
        id: Option<u64>,
        response: WsResponse,
    },
    Event(WsEvent),
}

/// An outgoing response to a [`WsCommand`].
#[derive(Debug, Clone, Serialize)]
pub enum WsResponse {
    /// Success
    Ok,
    /// The light stage's current operation mode
    Mode(StageMode),
    /// The server's config
    Config(ServerConfig),
    /// An error.
    Error { code: WsErrorKind, message: String },
}

/// Server-broadcast events ent to WebSocket clients.
#[derive(Debug, Clone, Serialize)]
pub enum WsEvent {
    /// Broadcast when light stage transitions to a new [`StageMode`]
    ModeChanged(StageMode),
    /// Broadcast when an active capture session completes.
    CaptureFinished,
}

impl From<StageEvent> for WsEvent {
    fn from(event: StageEvent) -> Self {
        match event {
            StageEvent::ModeChanged(stage_mode) => Self::ModeChanged(stage_mode),
            StageEvent::CaptureFinished => Self::CaptureFinished,
        }
    }
}

/// Error codes that can be returned
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WsErrorKind {
    InvalidPayload,
}

impl<E: ToString> From<Result<(), E>> for WsResponse {
    fn from(res: Result<(), E>) -> Self {
        match res {
            Ok(()) => WsResponse::Ok,
            Err(err) => WsResponse::Error {
                code: WsErrorKind::InvalidPayload,
                message: err.to_string(),
            },
        }
    }
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(api): State<ApiState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, api))
}

async fn handle_socket(mut socket: WebSocket, api: ApiState) {
    debug!("Websocket client connected.");

    let mut rx = { api.state.read().unwrap() }.tx.subscribe();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                if !handle_incoming_msg(msg, &mut socket, &api).await {
                    break;
                }
            }
            event = rx.recv() => {
                if !handle_broadcast_event(event, &mut socket).await {
                    break;
                }
            }
        }
    }
}

/// Process incoming raw WebSocket message
///
/// Return `true` if connection should remain open,
/// or `false` if disconnected or an unrecoverable error occured.
async fn handle_incoming_msg<E>(
    msg: Option<Result<Message, E>>,
    socket: &mut WebSocket,
    api: &ApiState,
) -> bool {
    let Some(Ok(msg)) = msg else {
        // websocket client disconnected?
        return false;
    };
    match msg {
        Message::Binary(bytes) => {
            let outbound = match ciborium::from_reader::<WsRequest, _>(&bytes[..]) {
                Ok(req) => {
                    let response = execute_command(req.command, api);
                    WsServerMessage::Response {
                        id: req.id,
                        response,
                    }
                }
                Err(e) => {
                    let err_response = WsResponse::Error {
                        code: WsErrorKind::InvalidPayload,
                        message: format!("Invalid CBOR payload: {e}"),
                    };
                    WsServerMessage::Response {
                        id: None,
                        response: err_response,
                    }
                }
            };
            if send_message(socket, &outbound).await.is_err() {
                error!("Failed to transmit websocket response! Dropping connection.");
                return false;
            }
        }
        Message::Close(_) => {
            debug!("Websocket client disconnected.");
            return false;
        }
        _ => {} // not a binary message, ignore.
    }
    true
}

/// Process system events from broadcast channel and sends them to connected clients.
///
/// Return `true` if connection should remain open,
/// or `false` if disconnected or an unrecoverable error occured.
async fn handle_broadcast_event<E>(event: Result<StageEvent, E>, socket: &mut WebSocket) -> bool {
    let Ok(stage_event) = event else {
        return false;
    };

    let outbound = WsServerMessage::Event(stage_event.into());
    if send_message(socket, &outbound).await.is_err() {
        return false;
    }
    true
}

/// Serialise outbound respones into CBOR message and send.
async fn send_message(socket: &mut WebSocket, message: &WsServerMessage) -> anyhow::Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    ciborium::into_writer(&message, &mut buf)?;
    socket.send(Message::Binary(buf.into())).await?;
    Ok(())
}

/// Interpret commands and update underlying state ([`ApiState`]).
fn execute_command(command: WsCommand, api: &ApiState) -> WsResponse {
    match command {
        WsCommand::GetConfig => WsResponse::Config(api.config),
        WsCommand::GetMode => WsResponse::Mode(api.get_mode()),
        WsCommand::SetMode(mode) => api.set_mode(mode).into(),
        WsCommand::SetFixture {
            arc_idx,
            light_idx,
            colour,
        } => api
            .set_fixture(arc_idx, light_idx, colour.rgb, colour.white)
            .into(),
        WsCommand::SetFixtures(fixtures) => {
            let mapped = fixtures
                .into_iter()
                .map(|req| (req.arc_idx, req.light_idx, req.colour.rgb, req.colour.white))
                .collect();
            api.set_fixtures(mapped).into()
        }
        WsCommand::SetArc { arc_idx, colour } => {
            api.set_arc(arc_idx, colour.rgb, colour.white).into()
        }
        WsCommand::SetLightstage(colour) => {
            api.set_lightstage(colour.rgb, colour.white);
            WsResponse::Ok
        }
        WsCommand::ManualTrigger => api.trigger_manual().into(),
    }
}
