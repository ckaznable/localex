use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum FileRequestPayload {
    Done,
    Chunk {
        data: Vec<u8>,
        offset: usize,
    },
    Ready {
        size: usize,
        chunks: usize,
        chunk_size: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExFileRequest {
    pub id: String,
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
    Ready,
    RequestChunk {
        offset: usize,
        end: usize,
    },
    RequestFile,
    Checked {
        result: ChunkResult,
        offset: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalExFileResponse {
    pub id: String,
    pub session: String,
    pub payload: FileResponsePayload,
}
