#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FFIError {
    #[error("init error")]
    InitError,
    #[error("access service before init error")]
    AccessServiceBeforeInitError,
    #[error("create service error")]
    CreateServiceError,
    #[error("get service error")]
    GetServiceError,
    #[error("get daemon channel error")]
    GetDaemonChannelError,
    #[error("listen libp2p error")]
    ListenLibP2PError,
    #[error("ffi convert error")]
    FFIConvertError,
    #[error("ffi channel error")]
    FFIChannelError,
    #[error("unknown error")]
    Unknown,
}
