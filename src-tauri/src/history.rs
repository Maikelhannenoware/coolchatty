use std::str::FromStr;

use directories::ProjectDirs;
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    FromRow, Pool, Sqlite,
};
use tracing::info;

use crate::errors::{AppError, AppResult};

#[derive(Debug, Serialize, FromRow)]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub created_at: String,
}

pub struct HistoryStore {
    pool: Pool<Sqlite>,
}

impl HistoryStore {
    pub async fn new() -> AppResult<Self> {
        let dirs = ProjectDirs::from("com", "coolchatty", "CoolChatty")
            .ok_or_else(|| AppError::History("unable to locate data directory".into()))?;
        let db_path = dirs.data_local_dir().join("history.db");
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| AppError::History(err.to_string()))?;
        }
        info!(path = %db_path.display(), "history database");
        let encoded_path = db_path.to_string_lossy().replace(' ', "%20");
        let conn_str = format!("sqlite://{encoded_path}");
        let options = SqliteConnectOptions::from_str(&conn_str)
            .map_err(|err| AppError::History(err.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|err| AppError::History(err.to_string()))?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|err| AppError::History(err.to_string()))?;
        Ok(Self { pool })
    }

    pub async fn add(&self, text: &str) -> AppResult<()> {
        sqlx::query("INSERT INTO history (text) VALUES (?1)")
            .bind(text)
            .execute(&self.pool)
            .await
            .map_err(|err| AppError::History(err.to_string()))?;
        Ok(())
    }

    pub async fn all(&self) -> AppResult<Vec<HistoryEntry>> {
        sqlx::query_as::<_, HistoryEntry>(
            "SELECT id, text, created_at FROM history ORDER BY id DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| AppError::History(err.to_string()))
    }

    pub async fn clear(&self) -> AppResult<()> {
        sqlx::query("DELETE FROM history")
            .execute(&self.pool)
            .await
            .map_err(|err| AppError::History(err.to_string()))?;
        Ok(())
    }
}
