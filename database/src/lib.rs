use anyhow::anyhow;
use anyhow::Result;
use sea_orm::Condition;
use sea_orm::QueryFilter;
use sea_orm::entity::{ColumnTrait, ModelTrait};
use sea_orm::Set;
use std::path::PathBuf;
use entity::regist_file;
use entity::regist_file::Entity as RegistFile;
use entity::regist_file::Model as RegistFileModel;

use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection, EntityTrait};

pub struct LocalExDb {
    db: DatabaseConnection,
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

    pub async fn get_all_regist_files(&self) -> Result<Vec<RegistFileModel>> {
        RegistFile::find().all(&self.db).await.map_err(anyhow::Error::from)
    }

    pub async fn get_all_regist_files_by_app(&self, app_id: &str) -> Result<Vec<RegistFileModel>> {
        RegistFile::find()
            .filter(regist_file::Column::AppId.eq(app_id))
            .all(&self.db)
            .await.map_err(anyhow::Error::from)
    }

    pub async fn get_file_model(&self, app_id: &str, file_id: &str) -> Option<RegistFileModel> {
        RegistFile::find()
            .filter(
                Condition::all()
                    .add(regist_file::Column::FileId.eq(file_id))
                    .add(regist_file::Column::AppId.eq(app_id))
            )
            .one(&self.db)
            .await
            .ok()
            .flatten()
    }

    pub async fn regist_file(&self, app_id: &str, file_id: &str, path: &str) -> Result<()> {
        if self.get_file_model(app_id, file_id).await.is_some() {
            return Err(anyhow!("already exists: {} {}", app_id, file_id));
        }

        let pear = regist_file::ActiveModel {
            app_id: Set(app_id.to_owned()),
            file_id: Set(file_id.to_owned()),
            path: Set(path.to_owned()),
            ..Default::default()
        };

        RegistFile::insert(pear).exec(&self.db).await?;
        Ok(())
    }

    pub async fn unregist_file(&self, app_id: &str, file_id: &str) -> Result<()> {
        if let Some(f) = self.get_file_model(app_id, file_id).await {
            f.delete(&self.db).await?;
        }

        Ok(())
    }

    pub async fn unregist_app(&self, app_id: &str) -> Result<()> {
        RegistFile::delete_many()
            .filter(regist_file::Column::AppId.eq(app_id))
            .exec(&self.db)
            .await?;

        Ok(())
    }

    pub async fn update_version(&self, app_id: &str, file_id: &str, version: i32) -> Result<()> {
        let pear = regist_file::ActiveModel {
            app_id: Set(app_id.to_owned()),
            file_id: Set(file_id.to_owned()),
            version: Set(version),
            ..Default::default()
        };

        RegistFile::update(pear).exec(&self.db).await?;
        Ok(())
    }

    pub async fn update_path(&self, app_id: &str, file_id: &str, path: &str) -> Result<()> {
        let pear = regist_file::ActiveModel {
            app_id: Set(app_id.to_owned()),
            file_id: Set(file_id.to_owned()),
            path: Set(path.to_owned()),
            ..Default::default()
        };

        RegistFile::update(pear).exec(&self.db).await?;
        Ok(())
    }

    pub async fn get_file_path(&self, app_id: &str, file_id: &str) -> Option<String> {
        self.get_file_model(app_id, file_id).await.map(|m| m.path)
    }
}
