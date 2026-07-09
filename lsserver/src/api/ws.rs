use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use serde::Deserialize;

use crate::api::{ApiState, UpdateColourRequest, UpdateFixturesRequest};

#[derive(Debug, Deserialize)]
pub enum WsCommand {
    SetFixture {
        arc_idx: usize,
        light_idx: usize,
        colour: UpdateColourRequest,
    },
    SetFixtures(Vec<UpdateFixturesRequest>),
    SetArc {
        arc_idx: usize,
        colour: UpdateColourRequest,
    },
    SetLightstage(UpdateColourRequest),
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(api): State<ApiState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, api))
}

async fn handle_socket(mut socket: WebSocket, api: ApiState) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Binary(bytes) = msg {
            match ciborium::from_reader::<WsCommand, _>(&bytes[..]) {
                Ok(cmd) => execute_command(cmd, &api),
                Err(_err) => todo!(),
            }
        }
    }
}

fn execute_command(command: WsCommand, api: &ApiState) {
    match command {
        WsCommand::SetFixture {
            arc_idx,
            light_idx,
            colour,
        } => api.set_fixture(arc_idx, light_idx, colour.rgb, colour.white),
        WsCommand::SetFixtures(fixtures) => {
            let mapped = fixtures
                .into_iter()
                .map(|req| (req.arc_idx, req.light_idx, req.colour.rgb, req.colour.white))
                .collect();
            api.set_fixtures(mapped);
        }
        WsCommand::SetArc { arc_idx, colour } => {
            api.set_arc(arc_idx, colour.rgb, colour.white);
        }
        WsCommand::SetLightstage(colour) => {
            api.set_lightstage(colour.rgb, colour.white);
        }
    }
}
