use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UrlRedirects::Table)
                    .if_not_exists()
                    .col(uuid(UrlRedirects::Id).primary_key())
                    .col(string(UrlRedirects::UserEmail))
                    .col(string(UrlRedirects::Key).unique_key())
                    .col(string(UrlRedirects::Target))
                    .col(
                        timestamp_with_time_zone(UrlRedirects::CreatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp_with_time_zone(UrlRedirects::UpdatedAt)
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UrlRedirects::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UrlRedirects {
    Table,
    Id,
    UserEmail,
    Key,
    Target,
    CreatedAt,
    UpdatedAt,
}
