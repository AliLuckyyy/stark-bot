use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, Result as SqliteResult};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::{ApiKey, Session};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(database_url: &str) -> SqliteResult<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = Path::new(database_url).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }

        let conn = Connection::open(database_url)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token TEXT UNIQUE NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            )",
            [],
        )?;

        // External API keys table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS external_api_keys (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT UNIQUE NOT NULL,
                api_key TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    // Session methods
    pub fn create_session(&self) -> SqliteResult<Session> {
        let conn = self.conn.lock().unwrap();
        let token = Uuid::new_v4().to_string();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(24);

        conn.execute(
            "INSERT INTO sessions (token, created_at, expires_at) VALUES (?1, ?2, ?3)",
            [
                &token,
                &created_at.to_rfc3339(),
                &expires_at.to_rfc3339(),
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Session {
            id,
            token,
            created_at,
            expires_at,
        })
    }

    pub fn validate_session(&self, token: &str) -> SqliteResult<Option<Session>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, token, created_at, expires_at FROM sessions WHERE token = ?1 AND expires_at > ?2",
        )?;

        let session = stmt
            .query_row([token, &now], |row| {
                let created_at_str: String = row.get(2)?;
                let expires_at_str: String = row.get(3)?;

                Ok(Session {
                    id: row.get(0)?,
                    token: row.get(1)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    expires_at: DateTime::parse_from_rfc3339(&expires_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        Ok(session)
    }

    pub fn delete_session(&self, token: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM sessions WHERE token = ?1", [token])?;
        Ok(rows_affected > 0)
    }

    // API Key methods
    pub fn get_api_key(&self, service_name: &str) -> SqliteResult<Option<ApiKey>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, service_name, api_key, created_at, updated_at FROM external_api_keys WHERE service_name = ?1",
        )?;

        let api_key = stmt
            .query_row([service_name], |row| {
                let created_at_str: String = row.get(3)?;
                let updated_at_str: String = row.get(4)?;

                Ok(ApiKey {
                    id: row.get(0)?,
                    service_name: row.get(1)?,
                    api_key: row.get(2)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        Ok(api_key)
    }

    pub fn list_api_keys(&self) -> SqliteResult<Vec<ApiKey>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, service_name, api_key, created_at, updated_at FROM external_api_keys ORDER BY service_name",
        )?;

        let api_keys = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(3)?;
                let updated_at_str: String = row.get(4)?;

                Ok(ApiKey {
                    id: row.get(0)?,
                    service_name: row.get(1)?,
                    api_key: row.get(2)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(api_keys)
    }

    pub fn upsert_api_key(&self, service_name: &str, api_key: &str) -> SqliteResult<ApiKey> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Try to update first
        let rows_affected = conn.execute(
            "UPDATE external_api_keys SET api_key = ?1, updated_at = ?2 WHERE service_name = ?3",
            [api_key, &now, service_name],
        )?;

        if rows_affected == 0 {
            // Insert new
            conn.execute(
                "INSERT INTO external_api_keys (service_name, api_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                [service_name, api_key, &now, &now],
            )?;
        }

        drop(conn);

        // Return the upserted key
        self.get_api_key(service_name).map(|opt| opt.unwrap())
    }

    pub fn delete_api_key(&self, service_name: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM external_api_keys WHERE service_name = ?1",
            [service_name],
        )?;
        Ok(rows_affected > 0)
    }
}
