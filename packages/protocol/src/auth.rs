use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AuthResponseState {
    Accept,
    Deny,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExAuthRequest {
    pub hostname: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExAuthResponse {
    pub state: AuthResponseState,
    pub hostname: String,
}
