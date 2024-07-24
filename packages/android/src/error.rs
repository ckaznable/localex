#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FFIError {
    #[error("init error")]
    InitError,
    #[error("ffi convert error")]
    FFIConvertError,
    #[error("ffi channel error")]
    FFIChannelError,
    #[error("unknown error")]
    Unknown,
}
