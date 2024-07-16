use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct IPCEventRequest {
    pub event: IPCEvent,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct IPCEventResponse {
    pub event: IPCEvent,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum IPCEvent {}
