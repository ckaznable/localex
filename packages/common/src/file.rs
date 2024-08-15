use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum FileRequestPayload {
    Done {
        id: String,
    },
    Chunk {
        id: String,
        data: Vec<u8>,
        offset: usize,
    },
    Ready {
        id: String,
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
    Ready {
        id: String,
    },
    RequestChunk {
        id: String,
        offset: usize,
        end: usize,
    },
    RequestFile {
        id: String,
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
