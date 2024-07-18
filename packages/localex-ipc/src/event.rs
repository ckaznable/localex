use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IPCEventRequest<T> {
    pub event: T,
    pub client_id: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IPCEventResponse<T> {
    pub event: T,
}

impl<T> IPCEventResponse<T> {
    pub fn from(event: T) -> Self {
        Self { event }
    }
}
