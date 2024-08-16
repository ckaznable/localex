use anyhow::{anyhow, Result};
use std::{collections::HashMap, io::SeekFrom, path::PathBuf, sync::Arc};

use common::BitVec;
use tokio::{fs::File, io::{AsyncSeekExt, AsyncWriteExt}, sync::RwLock};

#[derive(Default)]
pub struct FileHandleManager {
    map: HashMap<String, HashMap<String, Arc<RwLock<FileHandler>>>>,
}

impl FileHandleManager {
    pub async fn add(&mut self, session: String, id: String, size: usize, chunk_size: usize) -> Result<()> {
        match self.map.get_mut(&session) {
            None => {
                self.map.insert(session, HashMap::new());
            }
            Some(ids) => {
                if ids.contains_key(&id) {
                    return Err(anyhow!("id exists"));
                }

                let handler = FileHandler::new(size, chunk_size).await?;
                ids.insert(id, Arc::new(RwLock::new(handler)));
            }
        }

        Ok(())
    }

    pub async fn get(&self, session: &str, id: &str) -> Option<Arc<RwLock<FileHandler>>> {
        self.map.get(session).and_then(|ids| ids.get(id)).cloned()
    }
}

pub struct FileHandler {
    pub file_path: PathBuf,
    file: Option<File>,
    read_marker: BitVec,
    chunk_size: usize,
    size: usize,
}

impl FileHandler {
    pub async fn new(size: usize, chunk_size: usize) -> Result<Self> {
        let filename = format!("localex-tmp-{}", uuid::Uuid::new_v4());
        let path = dirs::cache_dir()
            .unwrap_or(PathBuf::from("./"))
            .join(filename);
        let file = File::create(&path).await?;

        Ok(Self {
            read_marker: BitVec::with_capacity((size / chunk_size).max(1)),
            file: Some(file),
            file_path: path,
            size,
            chunk_size,
        })
    }

    pub async fn write(&mut self, chunk: &[u8], offset: usize) -> Result<()> {
        let start = offset * self.chunk_size;
        let end = start + self.chunk_size;

        if start > self.size || end > self.size {
            return Err(anyhow!("oversized read"));
        }

        if let Some(file) = &mut self.file {
            file.seek(SeekFrom::Start(start as u64)).await?;
            file.write_all(chunk).await?;
            self.read_marker.set(offset, true);
        }

        Ok(())
    }

    pub fn done(&mut self) -> Option<Vec<(usize, usize)>> {
        if self.read_marker.all() {
            if let Some(a) = self.file.take() { drop(a) }
            None
        } else {
            Some(self.read_marker.get_zero_ranges())
        }
    }
}
