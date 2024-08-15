use anyhow::{anyhow, Result};
use std::{collections::HashMap, sync::Arc};

use common::BitVec;
use libp2p::bytes::BytesMut;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct FileReaderManager {
    map: HashMap<String, HashMap<String, Arc<RwLock<FileReader>>>>,
}

impl FileReaderManager {
    pub async fn add(&mut self, session: String, id: String, size: usize, chunk_size: usize) {
        match self.map.get_mut(&session) {
            None => {
                self.map.insert(session, HashMap::new());
            }
            Some(ids) => {
                ids.entry(id)
                    .or_insert_with(|| Arc::new(RwLock::new(FileReader::new(size, chunk_size))));
            }
        }
    }

    pub async fn get_reader(&self, session: &str, id: &str) -> Option<Arc<RwLock<FileReader>>> {
        self.map.get(session).and_then(|ids| ids.get(id)).cloned()
    }
}

pub struct FileReader {
    buf: BytesMut,
    read_marker: BitVec,
    chunk_size: usize,
}

impl FileReader {
    pub fn new(size: usize, chunk_size: usize) -> Self {
        Self {
            buf: BytesMut::with_capacity(size),
            read_marker: BitVec::with_capacity((size / chunk_size).max(1)),
            chunk_size,
        }
    }

    pub fn read(&mut self, chunk: &[u8], offset: usize) -> Result<()> {
        let start = offset * self.chunk_size;
        let end = start + self.chunk_size;
        let max = self.buf.len();

        if start > max || end > max {
            return Err(anyhow!("oversized read"));
        }

        self.buf[start..end].copy_from_slice(chunk);
        self.read_marker.set(offset, true);

        Ok(())
    }

    pub fn done(&self) -> Option<Vec<(usize, usize)>> {
        if self.read_marker.all() {
            None
        } else {
            Some(self.read_marker.get_zero_ranges())
        }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buf.to_vec()
    }
}
