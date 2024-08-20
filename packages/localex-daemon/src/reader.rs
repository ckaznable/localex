use anyhow::{anyhow, Result};
use std::{collections::HashMap, io::SeekFrom, path::PathBuf, sync::Arc};

use tokio::{fs::File, io::{AsyncSeekExt, AsyncWriteExt}, sync::RwLock};

#[derive(Default)]
pub struct FileHandleManager {
    map: HashMap<String, HashMap<String, Arc<RwLock<FileHandler>>>>,
}

impl FileHandleManager {
    pub async fn add(&mut self, session: String, id: String, size: usize, chunk_size: usize) -> Result<()> {
        let handler = FileHandler::new(size, chunk_size).await?;
        let handler = Arc::new(RwLock::new(handler));

        match self.map.get_mut(&session) {
            None => {
                let mut ids = HashMap::new();
                ids.insert(id, handler);
                self.map.insert(session, ids);
            }
            Some(ids) => {
                ids.insert(id, handler);
            }
        }

        Ok(())
    }

    pub fn get(&self, session: &str, id: &str) -> Option<Arc<RwLock<FileHandler>>> {
        self.map.get(session).and_then(|ids| ids.get(id)).cloned()
    }

    pub fn remove(&mut self, session: &str, id: &str) {
        if !self.map.contains_key(session) {
            return;
        }

        let ids = self.map.get_mut(session).unwrap();
        ids.remove(id);

        if ids.is_empty() {
            self.map.remove(session);
        }
    }
}

pub struct FileHandler {
    file_path: PathBuf,
    file: Option<File>,
    chunk_size: usize,
    size: usize,
}

impl FileHandler {
    pub async fn new(size: usize, chunk_size: usize) -> Result<Self> {
        let filename = format!("localex-tmp-{}", uuid::Uuid::new_v4());
        let path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("./"))
            .join(filename);
        let file = File::create(&path).await?;

        Ok(Self {
            file_path: path,
            file: Some(file),
            size,
            chunk_size,
        })
    }

    pub async fn write(&mut self, chunk: &[u8], offset: usize) -> Result<()> {
        let start = offset;
        let end = start + self.chunk_size;

        if start > self.size || end > self.size {
            return Err(anyhow!("oversized read"));
        }

        if let Some(file) = &mut self.file {
            file.seek(SeekFrom::Start(start as u64)).await?;
            file.write_all(chunk).await?;
        }

        Ok(())
    }

    pub fn get_file_path(&mut self) -> Result<PathBuf> {
        if let Some(file) = self.file.take() {
            drop(file);
        }

        Ok(self.file_path.clone())
    }
}
