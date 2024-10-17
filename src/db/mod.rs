use sqlx::{postgres::PgPoolOptions, Error, Executor, PgPool};
use thiserror::Error;

pub mod models;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Failed to parse database URL: {0}")]
    UrlParse(String),
    #[error("Database error: {0}")]
    Sqlx(#[from] Error),
    #[error("Failed to create database: {0}")]
    CreateDb(String),
}

pub async fn init_db(database_url: &str) -> Result<PgPool, DatabaseError> {
    let (base_url, db_name) = parse_database_url(database_url)?;

    let temp_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&base_url)
        .await
        .map_err(DatabaseError::Sqlx)?;

    ensure_database_exists(&temp_pool, &db_name).await?;

    PgPool::connect(database_url)
        .await
        .map_err(DatabaseError::Sqlx)
}

fn parse_database_url(database_url: &str) -> Result<(String, String), DatabaseError> {
    let base_url = database_url
        .rsplit_once('/')
        .ok_or_else(|| DatabaseError::UrlParse("Invalid database URL format".to_string()))?
        .0
        .to_string();

    let db_name = database_url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .ok_or_else(|| DatabaseError::UrlParse("Failed to extract database name".to_string()))?
        .to_string();

    Ok((base_url, db_name))
}

async fn ensure_database_exists(pool: &PgPool, db_name: &str) -> Result<(), DatabaseError> {
    let db_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(db_name)
            .fetch_one(pool)
            .await
            .map_err(DatabaseError::Sqlx)?;

    if !db_exists {
        pool.execute(format!("CREATE DATABASE {}", db_name).as_str())
            .await
            .map_err(|e| DatabaseError::CreateDb(e.to_string()))?;
    }

    Ok(())
}
