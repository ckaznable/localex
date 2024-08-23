use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistFile::Table)
                    .if_not_exists()
                    .col(pk_auto(RegistFile::Id))
                    .col(string(RegistFile::AppId))
                    .col(string(RegistFile::Path))
                    .col(integer(RegistFile::Version))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RegistFile::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RegistFile {
    Table,
    Id,
    AppId,
    Path,
    Version,
}
