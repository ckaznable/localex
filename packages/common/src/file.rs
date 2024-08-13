use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum FileRequestPayload {
    Chunk {
        id: String,
        data: Vec<u8>,
        offset: usize,
    },
    Ready {
        id: String,
        filename: String,
        size: usize,
        chunks: usize,
        chunk_size: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExFileRequest {
    pub session: String,
    pub payload: FileRequestPayload,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChunkResult {
    Success,
    Fail,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FileResponsePayload {
    Done,
    Ready,
    RequestChunk {
        offset: usize,
        end: usize,
    },
    RequestFile {
        id: String,
        filename: String,
    },
    Checked {
        id: String,
        result: ChunkResult,
        offset: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExFileResponse {
    pub session: String,
    pub payload: FileResponsePayload,
}
