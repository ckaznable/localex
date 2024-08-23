use std::path::PathBuf;

use anyhow::Result;
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};

pub struct LocalExDb {
    pub db: DatabaseConnection,
}

impl LocalExDb {
    pub async fn new(path: Option<PathBuf>) -> Result<Self> {
        let path = path
            .or_else(dirs::data_dir)
            .map(|p| p.join("localex").join("db.sqlite"))
            .and_then(|p| {
                std::fs::create_dir_all(p.parent()?).ok()?;
                Some(p)
            })
            .map(|p| format!("sqlite://{}?mode=rwc", p.into_os_string().to_string_lossy()))
            .unwrap_or_else(|| "sqlite::memory".to_owned());
        let db: DatabaseConnection = Database::connect(path).await?;
        Migrator::up(&db, None).await?;

        Ok(Self {db})
    }
}
