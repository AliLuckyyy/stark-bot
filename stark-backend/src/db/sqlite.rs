use chrono::{DateTime, Duration, NaiveDate, Timelike, Utc};
use rusqlite::{Connection, Result as SqliteResult};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

use crate::models::{
    AgentSettings, ApiKey, Channel, ChatSession, IdentityLink, Memory, MemorySearchResult,
    MemoryType, MessageRole, ResetPolicy, Session, SessionMessage, SessionScope,
};

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

        // Migrate: rename sessions -> auth_sessions if the old table exists
        let old_table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if old_table_exists {
            conn.execute("ALTER TABLE sessions RENAME TO auth_sessions", [])?;
        }

        // Auth sessions table (renamed from sessions)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS auth_sessions (
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

        // External channels table (Telegram, Slack, etc.)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS external_channels (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_type TEXT NOT NULL,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                bot_token TEXT NOT NULL,
                app_token TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(channel_type, name)
            )",
            [],
        )?;

        // Agent settings table (AI provider configuration)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                api_key TEXT NOT NULL,
                model TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Chat sessions table - conversation context containers
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chat_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_key TEXT UNIQUE NOT NULL,
                agent_id TEXT,
                scope TEXT NOT NULL DEFAULT 'dm',
                channel_type TEXT NOT NULL,
                channel_id INTEGER NOT NULL,
                platform_chat_id TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                reset_policy TEXT NOT NULL DEFAULT 'daily',
                idle_timeout_minutes INTEGER,
                daily_reset_hour INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_activity_at TEXT NOT NULL,
                expires_at TEXT,
                context_tokens INTEGER NOT NULL DEFAULT 0,
                max_context_tokens INTEGER NOT NULL DEFAULT 100000,
                compaction_id INTEGER
            )",
            [],
        )?;

        // Migration: Add context management columns if they don't exist
        let _ = conn.execute("ALTER TABLE chat_sessions ADD COLUMN context_tokens INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE chat_sessions ADD COLUMN max_context_tokens INTEGER NOT NULL DEFAULT 100000", []);
        let _ = conn.execute("ALTER TABLE chat_sessions ADD COLUMN compaction_id INTEGER", []);

        // Session messages table - conversation transcripts
        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                user_id TEXT,
                user_name TEXT,
                platform_message_id TEXT,
                tokens_used INTEGER,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Identity links table - cross-channel user mapping
        conn.execute(
            "CREATE TABLE IF NOT EXISTS identity_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                identity_id TEXT NOT NULL,
                channel_type TEXT NOT NULL,
                platform_user_id TEXT NOT NULL,
                platform_user_name TEXT,
                is_verified INTEGER NOT NULL DEFAULT 0,
                verified_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(channel_type, platform_user_id)
            )",
            [],
        )?;

        // Memories table - daily logs and long-term memories
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_type TEXT NOT NULL,
                content TEXT NOT NULL,
                category TEXT,
                tags TEXT,
                importance INTEGER NOT NULL DEFAULT 5,
                identity_id TEXT,
                session_id INTEGER,
                source_channel_type TEXT,
                source_message_id TEXT,
                log_date TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                expires_at TEXT,
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE SET NULL
            )",
            [],
        )?;

        // FTS5 virtual table for full-text search on memories
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content,
                category,
                tags,
                content=memories,
                content_rowid=id
            )",
            [],
        )?;

        // Triggers to keep FTS in sync with memories table
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content, category, tags)
                VALUES (new.id, new.content, new.category, new.tags);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, category, tags)
                VALUES ('delete', old.id, old.content, old.category, old.tags);
            END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, category, tags)
                VALUES ('delete', old.id, old.content, old.category, old.tags);
                INSERT INTO memories_fts(rowid, content, category, tags)
                VALUES (new.id, new.content, new.category, new.tags);
            END",
            [],
        )?;

        // Tool configuration table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_id INTEGER,
                profile TEXT NOT NULL DEFAULT 'standard',
                allow_list TEXT NOT NULL DEFAULT '[]',
                deny_list TEXT NOT NULL DEFAULT '[]',
                allowed_groups TEXT NOT NULL DEFAULT '[\"web\", \"filesystem\", \"exec\"]',
                denied_groups TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(channel_id)
            )",
            [],
        )?;

        // Drop old installed_skills table if it exists (migration)
        conn.execute("DROP TABLE IF EXISTS installed_skills", [])?;

        // Skills table (database-backed skill storage)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                description TEXT NOT NULL,
                body TEXT NOT NULL,
                version TEXT NOT NULL DEFAULT '1.0.0',
                author TEXT,
                homepage TEXT,
                metadata TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                requires_tools TEXT NOT NULL DEFAULT '[]',
                requires_binaries TEXT NOT NULL DEFAULT '[]',
                arguments TEXT NOT NULL DEFAULT '{}',
                tags TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Migration: Add homepage and metadata columns if they don't exist
        let has_homepage: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('skills') WHERE name='homepage'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_homepage {
            conn.execute("ALTER TABLE skills ADD COLUMN homepage TEXT", [])?;
            conn.execute("ALTER TABLE skills ADD COLUMN metadata TEXT", [])?;
        }

        // Skill scripts table (Python/Bash scripts bundled with skills)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS skill_scripts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                code TEXT NOT NULL,
                language TEXT NOT NULL DEFAULT 'python',
                created_at TEXT NOT NULL,
                FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
                UNIQUE(skill_id, name)
            )",
            [],
        )?;

        // Tool execution audit log
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_executions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_id INTEGER NOT NULL,
                session_id INTEGER,
                tool_name TEXT NOT NULL,
                parameters TEXT NOT NULL,
                success INTEGER NOT NULL,
                result TEXT,
                duration_ms INTEGER,
                executed_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE SET NULL
            )",
            [],
        )?;

        // Create index for tool executions lookup
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_executions_channel ON tool_executions(channel_id, executed_at)",
            [],
        )?;

        // Cron jobs table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cron_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                schedule_type TEXT NOT NULL,
                schedule_value TEXT NOT NULL,
                timezone TEXT,
                session_mode TEXT NOT NULL DEFAULT 'isolated',
                message TEXT,
                system_event TEXT,
                channel_id INTEGER,
                deliver_to TEXT,
                deliver INTEGER NOT NULL DEFAULT 0,
                model_override TEXT,
                thinking_level TEXT,
                timeout_seconds INTEGER,
                delete_after_run INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'active',
                last_run_at TEXT,
                next_run_at TEXT,
                run_count INTEGER NOT NULL DEFAULT 0,
                error_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES external_channels(id) ON DELETE SET NULL
            )",
            [],
        )?;

        // Cron job runs history
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cron_job_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                success INTEGER NOT NULL DEFAULT 0,
                result TEXT,
                error TEXT,
                duration_ms INTEGER,
                FOREIGN KEY (job_id) REFERENCES cron_jobs(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Index for job runs lookup
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_cron_job_runs_job ON cron_job_runs(job_id, started_at DESC)",
            [],
        )?;

        // Heartbeat configuration table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS heartbeat_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_id INTEGER UNIQUE,
                interval_minutes INTEGER NOT NULL DEFAULT 30,
                target TEXT NOT NULL DEFAULT 'last',
                active_hours_start TEXT,
                active_hours_end TEXT,
                active_days TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_beat_at TEXT,
                next_beat_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (channel_id) REFERENCES external_channels(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Gmail integration configuration
        conn.execute(
            "CREATE TABLE IF NOT EXISTS gmail_configs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT UNIQUE NOT NULL,
                access_token TEXT NOT NULL,
                refresh_token TEXT NOT NULL,
                token_expires_at TEXT,
                watch_labels TEXT NOT NULL DEFAULT 'INBOX',
                project_id TEXT NOT NULL,
                topic_name TEXT NOT NULL,
                watch_expires_at TEXT,
                history_id TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                response_channel_id INTEGER,
                auto_reply INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    // Auth Session methods (for web login sessions)
    pub fn create_session(&self) -> SqliteResult<Session> {
        let conn = self.conn.lock().unwrap();
        let token = Uuid::new_v4().to_string();
        let created_at = Utc::now();
        let expires_at = created_at + Duration::hours(24);

        conn.execute(
            "INSERT INTO auth_sessions (token, created_at, expires_at) VALUES (?1, ?2, ?3)",
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
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, token, created_at, expires_at FROM auth_sessions WHERE token = ?1 AND expires_at > ?2",
        )?;

        let session = stmt
            .query_row([token, &now_str], |row| {
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

        // Extend session expiry on successful validation (keep active sessions alive)
        if session.is_some() {
            let new_expires = (now + Duration::hours(24)).to_rfc3339();
            let _ = conn.execute(
                "UPDATE auth_sessions SET expires_at = ?1 WHERE token = ?2",
                [&new_expires, token],
            );
        }

        Ok(session)
    }

    pub fn delete_session(&self, token: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM auth_sessions WHERE token = ?1", [token])?;
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

    // Channel methods
    pub fn create_channel(
        &self,
        channel_type: &str,
        name: &str,
        bot_token: &str,
        app_token: Option<&str>,
    ) -> SqliteResult<Channel> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO external_channels (channel_type, name, enabled, bot_token, app_token, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6)",
            rusqlite::params![channel_type, name, bot_token, app_token, &now, &now],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Channel {
            id,
            channel_type: channel_type.to_string(),
            name: name.to_string(),
            enabled: false,
            bot_token: bot_token.to_string(),
            app_token: app_token.map(|s| s.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    pub fn get_channel(&self, id: i64) -> SqliteResult<Option<Channel>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, created_at, updated_at
             FROM external_channels WHERE id = ?1",
        )?;

        let channel = stmt
            .query_row([id], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(Channel {
                    id: row.get(0)?,
                    channel_type: row.get(1)?,
                    name: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    bot_token: row.get(4)?,
                    app_token: row.get(5)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        Ok(channel)
    }

    pub fn list_channels(&self) -> SqliteResult<Vec<Channel>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, created_at, updated_at
             FROM external_channels ORDER BY channel_type, name",
        )?;

        let channels = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(Channel {
                    id: row.get(0)?,
                    channel_type: row.get(1)?,
                    name: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    bot_token: row.get(4)?,
                    app_token: row.get(5)?,
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

        Ok(channels)
    }

    pub fn list_enabled_channels(&self) -> SqliteResult<Vec<Channel>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, created_at, updated_at
             FROM external_channels WHERE enabled = 1 ORDER BY channel_type, name",
        )?;

        let channels = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(Channel {
                    id: row.get(0)?,
                    channel_type: row.get(1)?,
                    name: row.get(2)?,
                    enabled: row.get::<_, i32>(3)? != 0,
                    bot_token: row.get(4)?,
                    app_token: row.get(5)?,
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

        Ok(channels)
    }

    pub fn update_channel(
        &self,
        id: i64,
        name: Option<&str>,
        enabled: Option<bool>,
        bot_token: Option<&str>,
        app_token: Option<Option<&str>>,
    ) -> SqliteResult<Option<Channel>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Build dynamic update query
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_idx = 2;

        if name.is_some() {
            updates.push(format!("name = ?{}", param_idx));
            param_idx += 1;
        }
        if enabled.is_some() {
            updates.push(format!("enabled = ?{}", param_idx));
            param_idx += 1;
        }
        if bot_token.is_some() {
            updates.push(format!("bot_token = ?{}", param_idx));
            param_idx += 1;
        }
        if app_token.is_some() {
            updates.push(format!("app_token = ?{}", param_idx));
            param_idx += 1;
        }

        let sql = format!(
            "UPDATE external_channels SET {} WHERE id = ?{}",
            updates.join(", "),
            param_idx
        );

        // Build params dynamically
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

        if let Some(n) = name {
            params.push(Box::new(n.to_string()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(if e { 1 } else { 0 }));
        }
        if let Some(t) = bot_token {
            params.push(Box::new(t.to_string()));
        }
        if let Some(at) = app_token {
            params.push(Box::new(at.map(|s| s.to_string())));
        }
        params.push(Box::new(id));

        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_ref.as_slice())?;

        drop(conn);
        self.get_channel(id)
    }

    pub fn set_channel_enabled(&self, id: i64, enabled: bool) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let rows_affected = conn.execute(
            "UPDATE external_channels SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![if enabled { 1 } else { 0 }, &now, id],
        )?;

        Ok(rows_affected > 0)
    }

    pub fn delete_channel(&self, id: i64) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM external_channels WHERE id = ?1",
            [id],
        )?;
        Ok(rows_affected > 0)
    }

    // Agent Settings methods

    /// Get the currently enabled agent settings (only one can be enabled)
    pub fn get_active_agent_settings(&self) -> SqliteResult<Option<AgentSettings>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, provider, endpoint, api_key, model, enabled, created_at, updated_at
             FROM agent_settings WHERE enabled = 1 LIMIT 1",
        )?;

        let settings = stmt
            .query_row([], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(AgentSettings {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    endpoint: row.get(2)?,
                    api_key: row.get(3)?,
                    model: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        Ok(settings)
    }

    /// Get agent settings by provider name
    pub fn get_agent_settings_by_provider(&self, provider: &str) -> SqliteResult<Option<AgentSettings>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, provider, endpoint, api_key, model, enabled, created_at, updated_at
             FROM agent_settings WHERE provider = ?1",
        )?;

        let settings = stmt
            .query_row([provider], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(AgentSettings {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    endpoint: row.get(2)?,
                    api_key: row.get(3)?,
                    model: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        Ok(settings)
    }

    /// List all agent settings
    pub fn list_agent_settings(&self) -> SqliteResult<Vec<AgentSettings>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, provider, endpoint, api_key, model, enabled, created_at, updated_at
             FROM agent_settings ORDER BY provider",
        )?;

        let settings = stmt
            .query_map([], |row| {
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                Ok(AgentSettings {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    endpoint: row.get(2)?,
                    api_key: row.get(3)?,
                    model: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
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

        Ok(settings)
    }

    /// Save agent settings (upsert by provider, and set as the only enabled one)
    pub fn save_agent_settings(
        &self,
        provider: &str,
        endpoint: &str,
        api_key: &str,
        model: &str,
    ) -> SqliteResult<AgentSettings> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // First, disable all existing settings
        conn.execute("UPDATE agent_settings SET enabled = 0, updated_at = ?1", [&now])?;

        // Check if this provider already exists
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM agent_settings WHERE provider = ?1",
                [provider],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            // Update existing
            conn.execute(
                "UPDATE agent_settings SET endpoint = ?1, api_key = ?2, model = ?3, enabled = 1, updated_at = ?4 WHERE id = ?5",
                rusqlite::params![endpoint, api_key, model, &now, id],
            )?;
        } else {
            // Insert new
            conn.execute(
                "INSERT INTO agent_settings (provider, endpoint, api_key, model, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
                rusqlite::params![provider, endpoint, api_key, model, &now, &now],
            )?;
        }

        drop(conn);

        // Return the saved settings
        self.get_agent_settings_by_provider(provider)
            .map(|opt| opt.unwrap())
    }

    /// Disable all agent settings (no AI provider active)
    pub fn disable_agent_settings(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute("UPDATE agent_settings SET enabled = 0, updated_at = ?1", [&now])?;
        Ok(())
    }

    // ============================================
    // Chat Session methods
    // ============================================

    /// Generate a session key from channel info
    fn generate_session_key(channel_type: &str, channel_id: i64, platform_chat_id: &str) -> String {
        format!("{}:{}:{}", channel_type, channel_id, platform_chat_id)
    }

    /// Get or create a chat session, handling reset policy
    pub fn get_or_create_chat_session(
        &self,
        channel_type: &str,
        channel_id: i64,
        platform_chat_id: &str,
        scope: SessionScope,
        agent_id: Option<&str>,
    ) -> SqliteResult<ChatSession> {
        let session_key = Self::generate_session_key(channel_type, channel_id, platform_chat_id);
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Try to get existing session
        if let Some(mut session) = self.get_chat_session_by_key(&session_key)? {
            // Check if session needs reset based on policy
            let should_reset = match session.reset_policy {
                ResetPolicy::Daily => {
                    let reset_hour = session.daily_reset_hour.unwrap_or(0);
                    let last_activity = session.last_activity_at;
                    let last_day = last_activity.date_naive();
                    let today = now.date_naive();

                    if today > last_day {
                        // Check if we've passed the reset hour today
                        now.hour() >= reset_hour as u32
                    } else {
                        false
                    }
                }
                ResetPolicy::Idle => {
                    if let Some(timeout) = session.idle_timeout_minutes {
                        let idle_duration = now.signed_duration_since(session.last_activity_at);
                        idle_duration.num_minutes() > timeout as i64
                    } else {
                        false
                    }
                }
                ResetPolicy::Manual | ResetPolicy::Never => false,
            };

            if should_reset {
                // Reset the session
                self.reset_chat_session(session.id)?;
                session = self.get_chat_session(session.id)?.unwrap();
            } else {
                // Update last activity
                let conn = self.conn.lock().unwrap();
                conn.execute(
                    "UPDATE chat_sessions SET last_activity_at = ?1, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![&now_str, session.id],
                )?;
                session.last_activity_at = now;
                session.updated_at = now;
            }

            return Ok(session);
        }

        // Create new session
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO chat_sessions (session_key, agent_id, scope, channel_type, channel_id, platform_chat_id,
             is_active, reset_policy, idle_timeout_minutes, daily_reset_hour, created_at, updated_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?10, ?10)",
            rusqlite::params![
                &session_key,
                agent_id,
                scope.as_str(),
                channel_type,
                channel_id,
                platform_chat_id,
                ResetPolicy::default().as_str(),
                Option::<i32>::None,
                Some(0i32),
                &now_str,
            ],
        )?;

        let id = conn.last_insert_rowid();
        drop(conn);

        self.get_chat_session(id).map(|opt| opt.unwrap())
    }

    /// Get a chat session by ID
    pub fn get_chat_session(&self, id: i64) -> SqliteResult<Option<ChatSession>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, session_key, agent_id, scope, channel_type, channel_id, platform_chat_id,
             is_active, reset_policy, idle_timeout_minutes, daily_reset_hour,
             created_at, updated_at, last_activity_at, expires_at,
             context_tokens, max_context_tokens, compaction_id
             FROM chat_sessions WHERE id = ?1",
        )?;

        let session = stmt
            .query_row([id], |row| Self::row_to_chat_session(row))
            .ok();

        Ok(session)
    }

    /// List all chat sessions
    pub fn list_chat_sessions(&self) -> SqliteResult<Vec<ChatSession>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, session_key, agent_id, scope, channel_type, channel_id, platform_chat_id,
             is_active, reset_policy, idle_timeout_minutes, daily_reset_hour,
             created_at, updated_at, last_activity_at, expires_at,
             context_tokens, max_context_tokens, compaction_id
             FROM chat_sessions ORDER BY last_activity_at DESC LIMIT 100",
        )?;

        let sessions = stmt
            .query_map([], |row| Self::row_to_chat_session(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(sessions)
    }

    /// Get a chat session by session key
    pub fn get_chat_session_by_key(&self, session_key: &str) -> SqliteResult<Option<ChatSession>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, session_key, agent_id, scope, channel_type, channel_id, platform_chat_id,
             is_active, reset_policy, idle_timeout_minutes, daily_reset_hour,
             created_at, updated_at, last_activity_at, expires_at,
             context_tokens, max_context_tokens, compaction_id
             FROM chat_sessions WHERE session_key = ?1 AND is_active = 1",
        )?;

        let session = stmt
            .query_row([session_key], |row| Self::row_to_chat_session(row))
            .ok();

        Ok(session)
    }

    /// Reset a chat session (mark old as inactive, create new)
    pub fn reset_chat_session(&self, id: i64) -> SqliteResult<ChatSession> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Get the old session info
        let old_session: Option<(String, Option<String>, String, String, i64, String, String, Option<i32>, Option<i32>)> = conn
            .query_row(
                "SELECT session_key, agent_id, scope, channel_type, channel_id, platform_chat_id, reset_policy, idle_timeout_minutes, daily_reset_hour
                 FROM chat_sessions WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?)),
            )
            .ok();

        let Some((session_key, agent_id, scope, channel_type, channel_id, platform_chat_id, reset_policy, idle_timeout, daily_hour)) = old_session else {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        };

        // Mark old session as inactive
        conn.execute(
            "UPDATE chat_sessions SET is_active = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![&now, id],
        )?;

        // Create new session with same settings
        conn.execute(
            "INSERT INTO chat_sessions (session_key, agent_id, scope, channel_type, channel_id, platform_chat_id,
             is_active, reset_policy, idle_timeout_minutes, daily_reset_hour, created_at, updated_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10, ?10, ?10)",
            rusqlite::params![
                &session_key,
                agent_id,
                &scope,
                &channel_type,
                channel_id,
                &platform_chat_id,
                &reset_policy,
                idle_timeout,
                daily_hour,
                &now,
            ],
        )?;

        let new_id = conn.last_insert_rowid();
        drop(conn);

        self.get_chat_session(new_id).map(|opt| opt.unwrap())
    }

    /// Update session reset policy
    pub fn update_session_reset_policy(
        &self,
        id: i64,
        reset_policy: ResetPolicy,
        idle_timeout_minutes: Option<i32>,
        daily_reset_hour: Option<i32>,
    ) -> SqliteResult<Option<ChatSession>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE chat_sessions SET reset_policy = ?1, idle_timeout_minutes = ?2, daily_reset_hour = ?3, updated_at = ?4 WHERE id = ?5",
            rusqlite::params![reset_policy.as_str(), idle_timeout_minutes, daily_reset_hour, &now, id],
        )?;

        drop(conn);
        self.get_chat_session(id)
    }

    fn row_to_chat_session(row: &rusqlite::Row) -> rusqlite::Result<ChatSession> {
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;
        let last_activity_str: String = row.get(13)?;
        let expires_at_str: Option<String> = row.get(14)?;
        let scope_str: String = row.get(3)?;
        let reset_policy_str: String = row.get(8)?;

        Ok(ChatSession {
            id: row.get(0)?,
            session_key: row.get(1)?,
            agent_id: row.get(2)?,
            scope: SessionScope::from_str(&scope_str).unwrap_or_default(),
            channel_type: row.get(4)?,
            channel_id: row.get(5)?,
            platform_chat_id: row.get(6)?,
            is_active: row.get::<_, i32>(7)? != 0,
            reset_policy: ResetPolicy::from_str(&reset_policy_str).unwrap_or_default(),
            idle_timeout_minutes: row.get(9)?,
            daily_reset_hour: row.get(10)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
            last_activity_at: DateTime::parse_from_rfc3339(&last_activity_str)
                .unwrap()
                .with_timezone(&Utc),
            expires_at: expires_at_str.map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .unwrap()
                    .with_timezone(&Utc)
            }),
            context_tokens: row.get(15).unwrap_or(0),
            max_context_tokens: row.get(16).unwrap_or(100000),
            compaction_id: row.get(17).ok(),
        })
    }

    // ============================================
    // Session Message methods
    // ============================================

    /// Add a message to a session
    pub fn add_session_message(
        &self,
        session_id: i64,
        role: MessageRole,
        content: &str,
        user_id: Option<&str>,
        user_name: Option<&str>,
        platform_message_id: Option<&str>,
        tokens_used: Option<i32>,
    ) -> SqliteResult<SessionMessage> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        conn.execute(
            "INSERT INTO session_messages (session_id, role, content, user_id, user_name, platform_message_id, tokens_used, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                session_id,
                role.as_str(),
                content,
                user_id,
                user_name,
                platform_message_id,
                tokens_used,
                &now_str,
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(SessionMessage {
            id,
            session_id,
            role,
            content: content.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            user_name: user_name.map(|s| s.to_string()),
            platform_message_id: platform_message_id.map(|s| s.to_string()),
            tokens_used,
            created_at: now,
        })
    }

    /// Get all messages for a session
    pub fn get_session_messages(&self, session_id: i64) -> SqliteResult<Vec<SessionMessage>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, user_id, user_name, platform_message_id, tokens_used, created_at
             FROM session_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;

        let messages = stmt
            .query_map([session_id], |row| Self::row_to_session_message(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    /// Get recent messages for a session (limited)
    pub fn get_recent_session_messages(&self, session_id: i64, limit: i32) -> SqliteResult<Vec<SessionMessage>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, user_id, user_name, platform_message_id, tokens_used, created_at
             FROM session_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let mut messages: Vec<SessionMessage> = stmt
            .query_map(rusqlite::params![session_id, limit], |row| Self::row_to_session_message(row))?
            .filter_map(|r| r.ok())
            .collect();

        // Reverse to get chronological order
        messages.reverse();
        Ok(messages)
    }

    /// Count messages in a session
    pub fn count_session_messages(&self, session_id: i64) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM session_messages WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
    }

    fn row_to_session_message(row: &rusqlite::Row) -> rusqlite::Result<SessionMessage> {
        let created_at_str: String = row.get(8)?;
        let role_str: String = row.get(2)?;

        Ok(SessionMessage {
            id: row.get(0)?,
            session_id: row.get(1)?,
            role: MessageRole::from_str(&role_str).unwrap_or(MessageRole::User),
            content: row.get(3)?,
            user_id: row.get(4)?,
            user_name: row.get(5)?,
            platform_message_id: row.get(6)?,
            tokens_used: row.get(7)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
        })
    }

    // ============================================
    // Context Management methods
    // ============================================

    /// Update the context token count for a session
    pub fn update_session_context_tokens(&self, session_id: i64, context_tokens: i32) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE chat_sessions SET context_tokens = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![context_tokens, &now, session_id],
        )?;
        Ok(())
    }

    /// Set the compaction ID for a session (after compaction occurs)
    pub fn set_session_compaction(&self, session_id: i64, compaction_id: i64) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE chat_sessions SET compaction_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![compaction_id, &now, session_id],
        )?;
        Ok(())
    }

    /// Get oldest messages for compaction (excludes most recent messages)
    pub fn get_messages_for_compaction(&self, session_id: i64, keep_recent: i32) -> SqliteResult<Vec<SessionMessage>> {
        let conn = self.conn.lock().unwrap();

        // Get total count first
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM session_messages WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;

        let to_compact = (total as i32).saturating_sub(keep_recent);
        if to_compact <= 0 {
            return Ok(vec![]);
        }

        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, user_id, user_name, platform_message_id, tokens_used, created_at
             FROM session_messages WHERE session_id = ?1 ORDER BY created_at ASC LIMIT ?2",
        )?;

        let messages = stmt
            .query_map(rusqlite::params![session_id, to_compact], |row| Self::row_to_session_message(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(messages)
    }

    /// Delete old messages after compaction (keeps the most recent messages)
    pub fn delete_compacted_messages(&self, session_id: i64, keep_recent: i32) -> SqliteResult<i32> {
        let conn = self.conn.lock().unwrap();

        // Get IDs of messages to delete (all except the most recent)
        let deleted = conn.execute(
            "DELETE FROM session_messages WHERE session_id = ?1 AND id NOT IN (
                SELECT id FROM session_messages WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2
            )",
            rusqlite::params![session_id, keep_recent],
        )?;

        Ok(deleted as i32)
    }

    /// Get the compaction summary for a session (if any)
    pub fn get_session_compaction_summary(&self, session_id: i64) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();

        // First get the compaction_id from the session
        let compaction_id: Option<i64> = conn.query_row(
            "SELECT compaction_id FROM chat_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        ).ok().flatten();

        let Some(compaction_id) = compaction_id else {
            return Ok(None);
        };

        // Get the compaction memory content
        let content: Option<String> = conn.query_row(
            "SELECT content FROM memories WHERE id = ?1",
            [compaction_id],
            |row| row.get(0),
        ).ok();

        Ok(content)
    }

    // ============================================
    // Identity methods
    // ============================================

    /// Get or create an identity for a platform user
    pub fn get_or_create_identity(
        &self,
        channel_type: &str,
        platform_user_id: &str,
        platform_user_name: Option<&str>,
    ) -> SqliteResult<IdentityLink> {
        // Try to get existing
        if let Some(link) = self.get_identity_by_platform(channel_type, platform_user_id)? {
            // Update username if changed
            if platform_user_name.is_some() && link.platform_user_name.as_deref() != platform_user_name {
                let conn = self.conn.lock().unwrap();
                let now = Utc::now().to_rfc3339();
                conn.execute(
                    "UPDATE identity_links SET platform_user_name = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![platform_user_name, &now, link.id],
                )?;
            }
            return self.get_identity_by_platform(channel_type, platform_user_id).map(|opt| opt.unwrap());
        }

        // Create new identity
        let identity_id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        conn.execute(
            "INSERT INTO identity_links (identity_id, channel_type, platform_user_id, platform_user_name, is_verified, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            rusqlite::params![&identity_id, channel_type, platform_user_id, platform_user_name, &now_str],
        )?;

        let id = conn.last_insert_rowid();

        Ok(IdentityLink {
            id,
            identity_id,
            channel_type: channel_type.to_string(),
            platform_user_id: platform_user_id.to_string(),
            platform_user_name: platform_user_name.map(|s| s.to_string()),
            is_verified: false,
            verified_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Link an existing identity to a new platform
    pub fn link_identity(
        &self,
        identity_id: &str,
        channel_type: &str,
        platform_user_id: &str,
        platform_user_name: Option<&str>,
    ) -> SqliteResult<IdentityLink> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        conn.execute(
            "INSERT INTO identity_links (identity_id, channel_type, platform_user_id, platform_user_name, is_verified, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            rusqlite::params![identity_id, channel_type, platform_user_id, platform_user_name, &now_str],
        )?;

        let id = conn.last_insert_rowid();

        Ok(IdentityLink {
            id,
            identity_id: identity_id.to_string(),
            channel_type: channel_type.to_string(),
            platform_user_id: platform_user_id.to_string(),
            platform_user_name: platform_user_name.map(|s| s.to_string()),
            is_verified: false,
            verified_at: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get identity by platform credentials
    pub fn get_identity_by_platform(&self, channel_type: &str, platform_user_id: &str) -> SqliteResult<Option<IdentityLink>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, identity_id, channel_type, platform_user_id, platform_user_name, is_verified, verified_at, created_at, updated_at
             FROM identity_links WHERE channel_type = ?1 AND platform_user_id = ?2",
        )?;

        let link = stmt
            .query_row(rusqlite::params![channel_type, platform_user_id], |row| Self::row_to_identity_link(row))
            .ok();

        Ok(link)
    }

    /// Get all linked identities for an identity_id
    pub fn get_linked_identities(&self, identity_id: &str) -> SqliteResult<Vec<IdentityLink>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, identity_id, channel_type, platform_user_id, platform_user_name, is_verified, verified_at, created_at, updated_at
             FROM identity_links WHERE identity_id = ?1",
        )?;

        let links = stmt
            .query_map([identity_id], |row| Self::row_to_identity_link(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }

    /// List all identity links (unique identities)
    pub fn list_identities(&self) -> SqliteResult<Vec<IdentityLink>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, identity_id, channel_type, platform_user_id, platform_user_name, is_verified, verified_at, created_at, updated_at
             FROM identity_links ORDER BY updated_at DESC LIMIT 100",
        )?;

        let links = stmt
            .query_map([], |row| Self::row_to_identity_link(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(links)
    }

    fn row_to_identity_link(row: &rusqlite::Row) -> rusqlite::Result<IdentityLink> {
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;
        let verified_at_str: Option<String> = row.get(6)?;

        Ok(IdentityLink {
            id: row.get(0)?,
            identity_id: row.get(1)?,
            channel_type: row.get(2)?,
            platform_user_id: row.get(3)?,
            platform_user_name: row.get(4)?,
            is_verified: row.get::<_, i32>(5)? != 0,
            verified_at: verified_at_str.map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .unwrap()
                    .with_timezone(&Utc)
            }),
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
        })
    }

    // ============================================
    // Memory methods
    // ============================================

    /// Create a memory (daily_log or long_term)
    pub fn create_memory(
        &self,
        memory_type: MemoryType,
        content: &str,
        category: Option<&str>,
        tags: Option<&str>,
        importance: i32,
        identity_id: Option<&str>,
        session_id: Option<i64>,
        source_channel_type: Option<&str>,
        source_message_id: Option<&str>,
        log_date: Option<NaiveDate>,
        expires_at: Option<DateTime<Utc>>,
    ) -> SqliteResult<Memory> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let log_date_str = log_date.map(|d| d.to_string());
        let expires_at_str = expires_at.map(|dt| dt.to_rfc3339());

        conn.execute(
            "INSERT INTO memories (memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
            rusqlite::params![
                memory_type.as_str(),
                content,
                category,
                tags,
                importance,
                identity_id,
                session_id,
                source_channel_type,
                source_message_id,
                log_date_str,
                &now_str,
                expires_at_str,
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Memory {
            id,
            memory_type,
            content: content.to_string(),
            category: category.map(|s| s.to_string()),
            tags: tags.map(|s| s.to_string()),
            importance,
            identity_id: identity_id.map(|s| s.to_string()),
            session_id,
            source_channel_type: source_channel_type.map(|s| s.to_string()),
            source_message_id: source_message_id.map(|s| s.to_string()),
            log_date,
            created_at: now,
            updated_at: now,
            expires_at,
        })
    }

    /// Search memories using FTS5
    pub fn search_memories(
        &self,
        query: &str,
        memory_type: Option<MemoryType>,
        identity_id: Option<&str>,
        category: Option<&str>,
        min_importance: Option<i32>,
        limit: i32,
    ) -> SqliteResult<Vec<MemorySearchResult>> {
        let conn = self.conn.lock().unwrap();

        // Build the query with filters
        let mut sql = String::from(
            "SELECT m.id, m.memory_type, m.content, m.category, m.tags, m.importance, m.identity_id,
             m.session_id, m.source_channel_type, m.source_message_id, m.log_date,
             m.created_at, m.updated_at, m.expires_at, bm25(memories_fts) as rank
             FROM memories m
             JOIN memories_fts ON m.id = memories_fts.rowid
             WHERE memories_fts MATCH ?1",
        );

        let mut conditions: Vec<String> = Vec::new();
        if memory_type.is_some() {
            conditions.push("m.memory_type = ?2".to_string());
        }
        if identity_id.is_some() {
            conditions.push(format!("m.identity_id = ?{}", if memory_type.is_some() { 3 } else { 2 }));
        }
        if category.is_some() {
            let idx = 2 + (memory_type.is_some() as usize) + (identity_id.is_some() as usize);
            conditions.push(format!("m.category = ?{}", idx));
        }
        if min_importance.is_some() {
            let idx = 2 + (memory_type.is_some() as usize) + (identity_id.is_some() as usize) + (category.is_some() as usize);
            conditions.push(format!("m.importance >= ?{}", idx));
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY rank LIMIT ?");
        let limit_idx = 2 + (memory_type.is_some() as usize) + (identity_id.is_some() as usize)
            + (category.is_some() as usize) + (min_importance.is_some() as usize);
        sql = sql.replace("LIMIT ?", &format!("LIMIT ?{}", limit_idx));

        let mut stmt = conn.prepare(&sql)?;

        // Build params dynamically
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(query.to_string())];
        if let Some(mt) = memory_type {
            params.push(Box::new(mt.as_str().to_string()));
        }
        if let Some(iid) = identity_id {
            params.push(Box::new(iid.to_string()));
        }
        if let Some(cat) = category {
            params.push(Box::new(cat.to_string()));
        }
        if let Some(mi) = min_importance {
            params.push(Box::new(mi));
        }
        params.push(Box::new(limit));

        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let results = stmt
            .query_map(params_ref.as_slice(), |row| {
                let memory = Self::row_to_memory(row)?;
                let rank: f64 = row.get(14)?;
                Ok(MemorySearchResult {
                    memory: memory.into(),
                    rank,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Get today's daily logs
    pub fn get_todays_daily_logs(&self, identity_id: Option<&str>) -> SqliteResult<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();
        let today = Utc::now().date_naive().to_string();

        let sql = if identity_id.is_some() {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'daily_log' AND log_date = ?1 AND identity_id = ?2 ORDER BY created_at ASC"
        } else {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'daily_log' AND log_date = ?1 ORDER BY created_at ASC"
        };

        let mut stmt = conn.prepare(sql)?;

        let memories: Vec<Memory> = if let Some(iid) = identity_id {
            stmt.query_map(rusqlite::params![&today, iid], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([&today], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(memories)
    }

    /// Get long-term memories for an identity
    pub fn get_long_term_memories(&self, identity_id: Option<&str>, min_importance: Option<i32>, limit: i32) -> SqliteResult<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();
        let min_imp = min_importance.unwrap_or(0);

        let sql = if identity_id.is_some() {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'long_term' AND identity_id = ?1 AND importance >= ?2
             ORDER BY importance DESC, created_at DESC LIMIT ?3"
        } else {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'long_term' AND importance >= ?1
             ORDER BY importance DESC, created_at DESC LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql)?;

        let memories: Vec<Memory> = if let Some(iid) = identity_id {
            stmt.query_map(rusqlite::params![iid, min_imp, limit], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map(rusqlite::params![min_imp, limit], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(memories)
    }

    /// Get session summaries (past conversation summaries)
    pub fn get_session_summaries(&self, identity_id: Option<&str>, limit: i32) -> SqliteResult<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();

        let sql = if identity_id.is_some() {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'session_summary' AND identity_id = ?1
             ORDER BY created_at DESC LIMIT ?2"
        } else {
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories WHERE memory_type = 'session_summary'
             ORDER BY created_at DESC LIMIT ?1"
        };

        let mut stmt = conn.prepare(sql)?;

        let memories: Vec<Memory> = if let Some(iid) = identity_id {
            stmt.query_map(rusqlite::params![iid, limit], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([limit], |row| Self::row_to_memory(row))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(memories)
    }

    /// List all memories
    pub fn list_memories(&self) -> SqliteResult<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, memory_type, content, category, tags, importance, identity_id, session_id,
             source_channel_type, source_message_id, log_date, created_at, updated_at, expires_at
             FROM memories ORDER BY created_at DESC LIMIT 100",
        )?;

        let memories = stmt
            .query_map([], |row| Self::row_to_memory(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(memories)
    }

    /// Delete a memory
    pub fn delete_memory(&self, id: i64) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM memories WHERE id = ?1", [id])?;
        Ok(rows_affected > 0)
    }

    /// Cleanup expired memories
    pub fn cleanup_expired_memories(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let rows_affected = conn.execute(
            "DELETE FROM memories WHERE expires_at IS NOT NULL AND expires_at < ?1",
            [&now],
        )?;
        Ok(rows_affected as i64)
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;
        let expires_at_str: Option<String> = row.get(13)?;
        let log_date_str: Option<String> = row.get(10)?;
        let memory_type_str: String = row.get(1)?;

        Ok(Memory {
            id: row.get(0)?,
            memory_type: MemoryType::from_str(&memory_type_str).unwrap_or(MemoryType::DailyLog),
            content: row.get(2)?,
            category: row.get(3)?,
            tags: row.get(4)?,
            importance: row.get(5)?,
            identity_id: row.get(6)?,
            session_id: row.get(7)?,
            source_channel_type: row.get(8)?,
            source_message_id: row.get(9)?,
            log_date: log_date_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
            expires_at: expires_at_str.map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .unwrap()
                    .with_timezone(&Utc)
            }),
        })
    }

    // Tool Configuration methods

    /// Get global tool config (channel_id = NULL)
    pub fn get_global_tool_config(&self) -> SqliteResult<Option<crate::tools::ToolConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, profile, allow_list, deny_list, allowed_groups, denied_groups
             FROM tool_configs WHERE channel_id IS NULL"
        )?;

        let config = stmt
            .query_row([], |row| {
                let allow_list: String = row.get(3)?;
                let deny_list: String = row.get(4)?;
                let allowed_groups: String = row.get(5)?;
                let denied_groups: String = row.get(6)?;
                let profile_str: String = row.get(2)?;

                Ok(crate::tools::ToolConfig {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    profile: crate::tools::ToolProfile::from_str(&profile_str)
                        .unwrap_or_default(),
                    allow_list: serde_json::from_str(&allow_list).unwrap_or_default(),
                    deny_list: serde_json::from_str(&deny_list).unwrap_or_default(),
                    allowed_groups: serde_json::from_str(&allowed_groups).unwrap_or_default(),
                    denied_groups: serde_json::from_str(&denied_groups).unwrap_or_default(),
                })
            })
            .ok();

        Ok(config)
    }

    /// Get tool config for a specific channel
    pub fn get_channel_tool_config(&self, channel_id: i64) -> SqliteResult<Option<crate::tools::ToolConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, profile, allow_list, deny_list, allowed_groups, denied_groups
             FROM tool_configs WHERE channel_id = ?1"
        )?;

        let config = stmt
            .query_row([channel_id], |row| {
                let allow_list: String = row.get(3)?;
                let deny_list: String = row.get(4)?;
                let allowed_groups: String = row.get(5)?;
                let denied_groups: String = row.get(6)?;
                let profile_str: String = row.get(2)?;

                Ok(crate::tools::ToolConfig {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    profile: crate::tools::ToolProfile::from_str(&profile_str)
                        .unwrap_or_default(),
                    allow_list: serde_json::from_str(&allow_list).unwrap_or_default(),
                    deny_list: serde_json::from_str(&deny_list).unwrap_or_default(),
                    allowed_groups: serde_json::from_str(&allowed_groups).unwrap_or_default(),
                    denied_groups: serde_json::from_str(&denied_groups).unwrap_or_default(),
                })
            })
            .ok();

        Ok(config)
    }

    /// Get effective tool config for a channel (falls back to global if channel config doesn't exist)
    pub fn get_effective_tool_config(&self, channel_id: Option<i64>) -> SqliteResult<crate::tools::ToolConfig> {
        if let Some(cid) = channel_id {
            if let Some(config) = self.get_channel_tool_config(cid)? {
                return Ok(config);
            }
        }

        Ok(self.get_global_tool_config()?.unwrap_or_default())
    }

    /// Save tool config (upsert)
    pub fn save_tool_config(&self, config: &crate::tools::ToolConfig) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let profile_str = match &config.profile {
            crate::tools::ToolProfile::None => "none",
            crate::tools::ToolProfile::Minimal => "minimal",
            crate::tools::ToolProfile::Standard => "standard",
            crate::tools::ToolProfile::Messaging => "messaging",
            crate::tools::ToolProfile::Full => "full",
            crate::tools::ToolProfile::Custom => "custom",
        };

        let allow_list_json = serde_json::to_string(&config.allow_list).unwrap_or_default();
        let deny_list_json = serde_json::to_string(&config.deny_list).unwrap_or_default();
        let allowed_groups_json = serde_json::to_string(&config.allowed_groups).unwrap_or_default();
        let denied_groups_json = serde_json::to_string(&config.denied_groups).unwrap_or_default();

        if config.channel_id.is_some() {
            conn.execute(
                "INSERT INTO tool_configs (channel_id, profile, allow_list, deny_list, allowed_groups, denied_groups, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(channel_id) DO UPDATE SET
                    profile = excluded.profile,
                    allow_list = excluded.allow_list,
                    deny_list = excluded.deny_list,
                    allowed_groups = excluded.allowed_groups,
                    denied_groups = excluded.denied_groups,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    config.channel_id,
                    profile_str,
                    allow_list_json,
                    deny_list_json,
                    allowed_groups_json,
                    denied_groups_json,
                    now
                ],
            )?;
        } else {
            // Global config (channel_id = NULL) - need special handling
            conn.execute(
                "DELETE FROM tool_configs WHERE channel_id IS NULL",
                [],
            )?;
            conn.execute(
                "INSERT INTO tool_configs (channel_id, profile, allow_list, deny_list, allowed_groups, denied_groups, created_at, updated_at)
                 VALUES (NULL, ?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                rusqlite::params![
                    profile_str,
                    allow_list_json,
                    deny_list_json,
                    allowed_groups_json,
                    denied_groups_json,
                    now
                ],
            )?;
        }

        Ok(conn.last_insert_rowid())
    }

    // Tool Execution logging methods

    /// Log a tool execution
    pub fn log_tool_execution(&self, execution: &crate::tools::ToolExecution) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let params_json = serde_json::to_string(&execution.parameters).unwrap_or_default();

        conn.execute(
            "INSERT INTO tool_executions (channel_id, session_id, tool_name, parameters, success, result, duration_ms, executed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                execution.channel_id,
                None::<i64>, // session_id could be added if needed
                execution.tool_name,
                params_json,
                execution.success as i32,
                execution.result,
                execution.duration_ms,
                execution.executed_at
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get tool execution history for a channel
    pub fn get_tool_execution_history(
        &self,
        channel_id: i64,
        limit: i32,
        offset: i32,
    ) -> SqliteResult<Vec<crate::tools::ToolExecution>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, tool_name, parameters, success, result, duration_ms, executed_at
             FROM tool_executions WHERE channel_id = ?1 ORDER BY executed_at DESC LIMIT ?2 OFFSET ?3"
        )?;

        let executions: Vec<crate::tools::ToolExecution> = stmt
            .query_map(rusqlite::params![channel_id, limit, offset], |row| {
                let params_str: String = row.get(3)?;
                Ok(crate::tools::ToolExecution {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    tool_name: row.get(2)?,
                    parameters: serde_json::from_str(&params_str).unwrap_or_default(),
                    success: row.get::<_, i32>(4)? != 0,
                    result: row.get(5)?,
                    duration_ms: row.get(6)?,
                    executed_at: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(executions)
    }

    /// Get all tool execution history
    pub fn get_all_tool_execution_history(
        &self,
        limit: i32,
        offset: i32,
    ) -> SqliteResult<Vec<crate::tools::ToolExecution>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, tool_name, parameters, success, result, duration_ms, executed_at
             FROM tool_executions ORDER BY executed_at DESC LIMIT ?1 OFFSET ?2"
        )?;

        let executions: Vec<crate::tools::ToolExecution> = stmt
            .query_map(rusqlite::params![limit, offset], |row| {
                let params_str: String = row.get(3)?;
                Ok(crate::tools::ToolExecution {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    tool_name: row.get(2)?,
                    parameters: serde_json::from_str(&params_str).unwrap_or_default(),
                    success: row.get::<_, i32>(4)? != 0,
                    result: row.get(5)?,
                    duration_ms: row.get(6)?,
                    executed_at: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(executions)
    }

    // ============================================
    // Skills CRUD methods (database-backed)
    // ============================================

    /// Create a new skill in the database
    pub fn create_skill(&self, skill: &crate::skills::DbSkill) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let requires_tools_json = serde_json::to_string(&skill.requires_tools).unwrap_or_default();
        let requires_binaries_json = serde_json::to_string(&skill.requires_binaries).unwrap_or_default();
        let arguments_json = serde_json::to_string(&skill.arguments).unwrap_or_default();
        let tags_json = serde_json::to_string(&skill.tags).unwrap_or_default();

        conn.execute(
            "INSERT INTO skills (name, description, body, version, author, homepage, metadata, enabled, requires_tools, requires_binaries, arguments, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
             ON CONFLICT(name) DO UPDATE SET
                description = excluded.description,
                body = excluded.body,
                version = excluded.version,
                author = excluded.author,
                homepage = excluded.homepage,
                metadata = excluded.metadata,
                requires_tools = excluded.requires_tools,
                requires_binaries = excluded.requires_binaries,
                arguments = excluded.arguments,
                tags = excluded.tags,
                updated_at = excluded.updated_at",
            rusqlite::params![
                skill.name,
                skill.description,
                skill.body,
                skill.version,
                skill.author,
                skill.homepage,
                skill.metadata,
                skill.enabled as i32,
                requires_tools_json,
                requires_binaries_json,
                arguments_json,
                tags_json,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get a skill by name
    pub fn get_skill(&self, name: &str) -> SqliteResult<Option<crate::skills::DbSkill>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, body, version, author, homepage, metadata, enabled, requires_tools, requires_binaries, arguments, tags, created_at, updated_at
             FROM skills WHERE name = ?1"
        )?;

        let skill = stmt
            .query_row([name], |row| Self::row_to_db_skill(row))
            .ok();

        Ok(skill)
    }

    /// Get a skill by ID
    pub fn get_skill_by_id(&self, id: i64) -> SqliteResult<Option<crate::skills::DbSkill>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, body, version, author, homepage, metadata, enabled, requires_tools, requires_binaries, arguments, tags, created_at, updated_at
             FROM skills WHERE id = ?1"
        )?;

        let skill = stmt
            .query_row([id], |row| Self::row_to_db_skill(row))
            .ok();

        Ok(skill)
    }

    /// List all skills
    pub fn list_skills(&self) -> SqliteResult<Vec<crate::skills::DbSkill>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, body, version, author, homepage, metadata, enabled, requires_tools, requires_binaries, arguments, tags, created_at, updated_at
             FROM skills ORDER BY name"
        )?;

        let skills: Vec<crate::skills::DbSkill> = stmt
            .query_map([], |row| Self::row_to_db_skill(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    /// List enabled skills
    pub fn list_enabled_skills(&self) -> SqliteResult<Vec<crate::skills::DbSkill>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, body, version, author, homepage, metadata, enabled, requires_tools, requires_binaries, arguments, tags, created_at, updated_at
             FROM skills WHERE enabled = 1 ORDER BY name"
        )?;

        let skills: Vec<crate::skills::DbSkill> = stmt
            .query_map([], |row| Self::row_to_db_skill(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    /// Update skill enabled status
    pub fn set_skill_enabled(&self, name: &str, enabled: bool) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let rows_affected = conn.execute(
            "UPDATE skills SET enabled = ?1, updated_at = ?2 WHERE name = ?3",
            rusqlite::params![enabled as i32, now, name],
        )?;
        Ok(rows_affected > 0)
    }

    /// Delete a skill (cascade deletes scripts)
    pub fn delete_skill(&self, name: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM skills WHERE name = ?1",
            [name],
        )?;
        Ok(rows_affected > 0)
    }

    fn row_to_db_skill(row: &rusqlite::Row) -> rusqlite::Result<crate::skills::DbSkill> {
        let requires_tools_str: String = row.get(9)?;
        let requires_binaries_str: String = row.get(10)?;
        let arguments_str: String = row.get(11)?;
        let tags_str: String = row.get(12)?;

        Ok(crate::skills::DbSkill {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            body: row.get(3)?,
            version: row.get(4)?,
            // Handle NULL values for optional fields
            author: row.get::<_, Option<String>>(5)?,
            homepage: row.get::<_, Option<String>>(6)?,
            metadata: row.get::<_, Option<String>>(7)?,
            enabled: row.get::<_, i32>(8)? != 0,
            requires_tools: serde_json::from_str(&requires_tools_str).unwrap_or_default(),
            requires_binaries: serde_json::from_str(&requires_binaries_str).unwrap_or_default(),
            arguments: serde_json::from_str(&arguments_str).unwrap_or_default(),
            tags: serde_json::from_str(&tags_str).unwrap_or_default(),
            created_at: row.get(13)?,
            updated_at: row.get(14)?,
        })
    }

    // ============================================
    // Skill Scripts CRUD methods
    // ============================================

    /// Create a skill script
    pub fn create_skill_script(&self, script: &crate::skills::DbSkillScript) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO skill_scripts (skill_id, name, code, language, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(skill_id, name) DO UPDATE SET
                code = excluded.code,
                language = excluded.language",
            rusqlite::params![
                script.skill_id,
                script.name,
                script.code,
                script.language,
                now
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get all scripts for a skill
    pub fn get_skill_scripts(&self, skill_id: i64) -> SqliteResult<Vec<crate::skills::DbSkillScript>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, skill_id, name, code, language, created_at
             FROM skill_scripts WHERE skill_id = ?1 ORDER BY name"
        )?;

        let scripts: Vec<crate::skills::DbSkillScript> = stmt
            .query_map([skill_id], |row| {
                Ok(crate::skills::DbSkillScript {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    name: row.get(2)?,
                    code: row.get(3)?,
                    language: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(scripts)
    }

    /// Get scripts for a skill by skill name
    pub fn get_skill_scripts_by_name(&self, skill_name: &str) -> SqliteResult<Vec<crate::skills::DbSkillScript>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ss.id, ss.skill_id, ss.name, ss.code, ss.language, ss.created_at
             FROM skill_scripts ss
             JOIN skills s ON s.id = ss.skill_id
             WHERE s.name = ?1 ORDER BY ss.name"
        )?;

        let scripts: Vec<crate::skills::DbSkillScript> = stmt
            .query_map([skill_name], |row| {
                Ok(crate::skills::DbSkillScript {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    name: row.get(2)?,
                    code: row.get(3)?,
                    language: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(scripts)
    }

    /// Delete all scripts for a skill
    pub fn delete_skill_scripts(&self, skill_id: i64) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM skill_scripts WHERE skill_id = ?1",
            [skill_id],
        )?;
        Ok(rows_affected as i64)
    }

    // =============================================================
    // Cron Job Methods
    // =============================================================

    /// Create a new cron job
    pub fn create_cron_job(
        &self,
        name: &str,
        description: Option<&str>,
        schedule_type: &str,
        schedule_value: &str,
        timezone: Option<&str>,
        session_mode: &str,
        message: Option<&str>,
        system_event: Option<&str>,
        channel_id: Option<i64>,
        deliver_to: Option<&str>,
        deliver: bool,
        model_override: Option<&str>,
        thinking_level: Option<&str>,
        timeout_seconds: Option<i32>,
        delete_after_run: bool,
    ) -> SqliteResult<crate::models::CronJob> {
        let conn = self.conn.lock().unwrap();
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO cron_jobs (
                job_id, name, description, schedule_type, schedule_value, timezone,
                session_mode, message, system_event, channel_id, deliver_to, deliver,
                model_override, thinking_level, timeout_seconds, delete_after_run,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'active', ?17, ?17)",
            rusqlite::params![
                job_id, name, description, schedule_type, schedule_value, timezone,
                session_mode, message, system_event, channel_id, deliver_to, deliver as i32,
                model_override, thinking_level, timeout_seconds, delete_after_run as i32,
                now
            ],
        )?;

        let id = conn.last_insert_rowid();
        self.get_cron_job_by_id_internal(&conn, id)
    }

    fn get_cron_job_by_id_internal(&self, conn: &Connection, id: i64) -> SqliteResult<crate::models::CronJob> {
        conn.query_row(
            "SELECT id, job_id, name, description, schedule_type, schedule_value, timezone,
                    session_mode, message, system_event, channel_id, deliver_to, deliver,
                    model_override, thinking_level, timeout_seconds, delete_after_run,
                    status, last_run_at, next_run_at, run_count, error_count, last_error,
                    created_at, updated_at
             FROM cron_jobs WHERE id = ?1",
            [id],
            |row| self.map_cron_job_row(row),
        )
    }

    fn map_cron_job_row(&self, row: &rusqlite::Row) -> SqliteResult<crate::models::CronJob> {
        Ok(crate::models::CronJob {
            id: row.get(0)?,
            job_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            schedule_type: row.get(4)?,
            schedule_value: row.get(5)?,
            timezone: row.get(6)?,
            session_mode: row.get(7)?,
            message: row.get(8)?,
            system_event: row.get(9)?,
            channel_id: row.get(10)?,
            deliver_to: row.get(11)?,
            deliver: row.get::<_, i32>(12)? != 0,
            model_override: row.get(13)?,
            thinking_level: row.get(14)?,
            timeout_seconds: row.get(15)?,
            delete_after_run: row.get::<_, i32>(16)? != 0,
            status: row.get(17)?,
            last_run_at: row.get(18)?,
            next_run_at: row.get(19)?,
            run_count: row.get(20)?,
            error_count: row.get(21)?,
            last_error: row.get(22)?,
            created_at: row.get(23)?,
            updated_at: row.get(24)?,
        })
    }

    /// Get a cron job by ID
    pub fn get_cron_job(&self, id: i64) -> SqliteResult<Option<crate::models::CronJob>> {
        let conn = self.conn.lock().unwrap();
        match self.get_cron_job_by_id_internal(&conn, id) {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a cron job by job_id (UUID string)
    pub fn get_cron_job_by_job_id(&self, job_id: &str) -> SqliteResult<Option<crate::models::CronJob>> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT id, job_id, name, description, schedule_type, schedule_value, timezone,
                    session_mode, message, system_event, channel_id, deliver_to, deliver,
                    model_override, thinking_level, timeout_seconds, delete_after_run,
                    status, last_run_at, next_run_at, run_count, error_count, last_error,
                    created_at, updated_at
             FROM cron_jobs WHERE job_id = ?1",
            [job_id],
            |row| self.map_cron_job_row(row),
        ) {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// List all cron jobs
    pub fn list_cron_jobs(&self) -> SqliteResult<Vec<crate::models::CronJob>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, name, description, schedule_type, schedule_value, timezone,
                    session_mode, message, system_event, channel_id, deliver_to, deliver,
                    model_override, thinking_level, timeout_seconds, delete_after_run,
                    status, last_run_at, next_run_at, run_count, error_count, last_error,
                    created_at, updated_at
             FROM cron_jobs ORDER BY created_at DESC"
        )?;

        let jobs: Vec<crate::models::CronJob> = stmt
            .query_map([], |row| self.map_cron_job_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(jobs)
    }

    /// List active cron jobs that are due to run
    pub fn list_due_cron_jobs(&self) -> SqliteResult<Vec<crate::models::CronJob>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, job_id, name, description, schedule_type, schedule_value, timezone,
                    session_mode, message, system_event, channel_id, deliver_to, deliver,
                    model_override, thinking_level, timeout_seconds, delete_after_run,
                    status, last_run_at, next_run_at, run_count, error_count, last_error,
                    created_at, updated_at
             FROM cron_jobs
             WHERE status = 'active' AND (next_run_at IS NULL OR next_run_at <= ?1)
             ORDER BY next_run_at ASC"
        )?;

        let jobs: Vec<crate::models::CronJob> = stmt
            .query_map([&now], |row| self.map_cron_job_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(jobs)
    }

    /// Update a cron job
    pub fn update_cron_job(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        schedule_type: Option<&str>,
        schedule_value: Option<&str>,
        timezone: Option<&str>,
        session_mode: Option<&str>,
        message: Option<&str>,
        system_event: Option<&str>,
        channel_id: Option<i64>,
        deliver_to: Option<&str>,
        deliver: Option<bool>,
        model_override: Option<&str>,
        thinking_level: Option<&str>,
        timeout_seconds: Option<i32>,
        delete_after_run: Option<bool>,
        status: Option<&str>,
    ) -> SqliteResult<crate::models::CronJob> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Build dynamic update query
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_index = 2;

        if name.is_some() { updates.push(format!("name = ?{}", param_index)); param_index += 1; }
        if description.is_some() { updates.push(format!("description = ?{}", param_index)); param_index += 1; }
        if schedule_type.is_some() { updates.push(format!("schedule_type = ?{}", param_index)); param_index += 1; }
        if schedule_value.is_some() { updates.push(format!("schedule_value = ?{}", param_index)); param_index += 1; }
        if timezone.is_some() { updates.push(format!("timezone = ?{}", param_index)); param_index += 1; }
        if session_mode.is_some() { updates.push(format!("session_mode = ?{}", param_index)); param_index += 1; }
        if message.is_some() { updates.push(format!("message = ?{}", param_index)); param_index += 1; }
        if system_event.is_some() { updates.push(format!("system_event = ?{}", param_index)); param_index += 1; }
        if channel_id.is_some() { updates.push(format!("channel_id = ?{}", param_index)); param_index += 1; }
        if deliver_to.is_some() { updates.push(format!("deliver_to = ?{}", param_index)); param_index += 1; }
        if deliver.is_some() { updates.push(format!("deliver = ?{}", param_index)); param_index += 1; }
        if model_override.is_some() { updates.push(format!("model_override = ?{}", param_index)); param_index += 1; }
        if thinking_level.is_some() { updates.push(format!("thinking_level = ?{}", param_index)); param_index += 1; }
        if timeout_seconds.is_some() { updates.push(format!("timeout_seconds = ?{}", param_index)); param_index += 1; }
        if delete_after_run.is_some() { updates.push(format!("delete_after_run = ?{}", param_index)); param_index += 1; }
        if status.is_some() { updates.push(format!("status = ?{}", param_index)); param_index += 1; }

        let query = format!(
            "UPDATE cron_jobs SET {} WHERE id = ?{}",
            updates.join(", "),
            param_index
        );

        // Build params dynamically
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        if let Some(v) = name { params.push(Box::new(v.to_string())); }
        if let Some(v) = description { params.push(Box::new(v.to_string())); }
        if let Some(v) = schedule_type { params.push(Box::new(v.to_string())); }
        if let Some(v) = schedule_value { params.push(Box::new(v.to_string())); }
        if let Some(v) = timezone { params.push(Box::new(v.to_string())); }
        if let Some(v) = session_mode { params.push(Box::new(v.to_string())); }
        if let Some(v) = message { params.push(Box::new(v.to_string())); }
        if let Some(v) = system_event { params.push(Box::new(v.to_string())); }
        if let Some(v) = channel_id { params.push(Box::new(v)); }
        if let Some(v) = deliver_to { params.push(Box::new(v.to_string())); }
        if let Some(v) = deliver { params.push(Box::new(v as i32)); }
        if let Some(v) = model_override { params.push(Box::new(v.to_string())); }
        if let Some(v) = thinking_level { params.push(Box::new(v.to_string())); }
        if let Some(v) = timeout_seconds { params.push(Box::new(v)); }
        if let Some(v) = delete_after_run { params.push(Box::new(v as i32)); }
        if let Some(v) = status { params.push(Box::new(v.to_string())); }
        params.push(Box::new(id));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&query, params_refs.as_slice())?;

        self.get_cron_job_by_id_internal(&conn, id)
    }

    /// Update cron job run status
    pub fn update_cron_job_run_status(
        &self,
        id: i64,
        last_run_at: &str,
        next_run_at: Option<&str>,
        success: bool,
        error: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        if success {
            conn.execute(
                "UPDATE cron_jobs SET
                    last_run_at = ?1, next_run_at = ?2, run_count = run_count + 1,
                    last_error = NULL, updated_at = ?3
                 WHERE id = ?4",
                rusqlite::params![last_run_at, next_run_at, now, id],
            )?;
        } else {
            conn.execute(
                "UPDATE cron_jobs SET
                    last_run_at = ?1, next_run_at = ?2, error_count = error_count + 1,
                    last_error = ?3, updated_at = ?4
                 WHERE id = ?5",
                rusqlite::params![last_run_at, next_run_at, error, now, id],
            )?;
        }

        Ok(())
    }

    /// Delete a cron job
    pub fn delete_cron_job(&self, id: i64) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute("DELETE FROM cron_jobs WHERE id = ?1", [id])?;
        Ok(rows_affected > 0)
    }

    /// Log a cron job run
    pub fn log_cron_job_run(
        &self,
        job_id: i64,
        started_at: &str,
        completed_at: Option<&str>,
        success: bool,
        result: Option<&str>,
        error: Option<&str>,
        duration_ms: Option<i64>,
    ) -> SqliteResult<crate::models::CronJobRun> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO cron_job_runs (job_id, started_at, completed_at, success, result, error, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![job_id, started_at, completed_at, success as i32, result, error, duration_ms],
        )?;

        let id = conn.last_insert_rowid();

        Ok(crate::models::CronJobRun {
            id,
            job_id,
            started_at: started_at.to_string(),
            completed_at: completed_at.map(|s| s.to_string()),
            success,
            result: result.map(|s| s.to_string()),
            error: error.map(|s| s.to_string()),
            duration_ms,
        })
    }

    /// Get recent runs for a cron job
    pub fn get_cron_job_runs(&self, job_id: i64, limit: i32) -> SqliteResult<Vec<crate::models::CronJobRun>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, job_id, started_at, completed_at, success, result, error, duration_ms
             FROM cron_job_runs WHERE job_id = ?1 ORDER BY started_at DESC LIMIT ?2"
        )?;

        let runs: Vec<crate::models::CronJobRun> = stmt
            .query_map([job_id, limit as i64], |row| {
                Ok(crate::models::CronJobRun {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    started_at: row.get(2)?,
                    completed_at: row.get(3)?,
                    success: row.get::<_, i32>(4)? != 0,
                    result: row.get(5)?,
                    error: row.get(6)?,
                    duration_ms: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(runs)
    }

    // =============================================================
    // Heartbeat Config Methods
    // =============================================================

    /// Get or create heartbeat config for a channel (or global if channel_id is None)
    pub fn get_or_create_heartbeat_config(&self, channel_id: Option<i64>) -> SqliteResult<crate::models::HeartbeatConfig> {
        let conn = self.conn.lock().unwrap();

        // Try to get existing config
        let existing = if let Some(cid) = channel_id {
            conn.query_row(
                "SELECT id, channel_id, interval_minutes, target, active_hours_start, active_hours_end,
                        active_days, enabled, last_beat_at, next_beat_at, created_at, updated_at
                 FROM heartbeat_configs WHERE channel_id = ?1",
                [cid],
                |row| self.map_heartbeat_config_row(row),
            ).ok()
        } else {
            conn.query_row(
                "SELECT id, channel_id, interval_minutes, target, active_hours_start, active_hours_end,
                        active_days, enabled, last_beat_at, next_beat_at, created_at, updated_at
                 FROM heartbeat_configs WHERE channel_id IS NULL",
                [],
                |row| self.map_heartbeat_config_row(row),
            ).ok()
        };

        if let Some(config) = existing {
            return Ok(config);
        }

        // Create new config
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO heartbeat_configs (channel_id, interval_minutes, target, enabled, created_at, updated_at)
             VALUES (?1, 30, 'last', 1, ?2, ?2)",
            rusqlite::params![channel_id, now],
        )?;

        let id = conn.last_insert_rowid();

        Ok(crate::models::HeartbeatConfig {
            id,
            channel_id,
            interval_minutes: 30,
            target: "last".to_string(),
            active_hours_start: None,
            active_hours_end: None,
            active_days: None,
            enabled: true,
            last_beat_at: None,
            next_beat_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    fn map_heartbeat_config_row(&self, row: &rusqlite::Row) -> SqliteResult<crate::models::HeartbeatConfig> {
        Ok(crate::models::HeartbeatConfig {
            id: row.get(0)?,
            channel_id: row.get(1)?,
            interval_minutes: row.get(2)?,
            target: row.get(3)?,
            active_hours_start: row.get(4)?,
            active_hours_end: row.get(5)?,
            active_days: row.get(6)?,
            enabled: row.get::<_, i32>(7)? != 0,
            last_beat_at: row.get(8)?,
            next_beat_at: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }

    /// Update heartbeat config
    pub fn update_heartbeat_config(
        &self,
        id: i64,
        interval_minutes: Option<i32>,
        target: Option<&str>,
        active_hours_start: Option<&str>,
        active_hours_end: Option<&str>,
        active_days: Option<&str>,
        enabled: Option<bool>,
    ) -> SqliteResult<crate::models::HeartbeatConfig> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_index = 2;

        if interval_minutes.is_some() { updates.push(format!("interval_minutes = ?{}", param_index)); param_index += 1; }
        if target.is_some() { updates.push(format!("target = ?{}", param_index)); param_index += 1; }
        if active_hours_start.is_some() { updates.push(format!("active_hours_start = ?{}", param_index)); param_index += 1; }
        if active_hours_end.is_some() { updates.push(format!("active_hours_end = ?{}", param_index)); param_index += 1; }
        if active_days.is_some() { updates.push(format!("active_days = ?{}", param_index)); param_index += 1; }
        if enabled.is_some() { updates.push(format!("enabled = ?{}", param_index)); param_index += 1; }

        let query = format!(
            "UPDATE heartbeat_configs SET {} WHERE id = ?{}",
            updates.join(", "),
            param_index
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        if let Some(v) = interval_minutes { params.push(Box::new(v)); }
        if let Some(v) = target { params.push(Box::new(v.to_string())); }
        if let Some(v) = active_hours_start { params.push(Box::new(v.to_string())); }
        if let Some(v) = active_hours_end { params.push(Box::new(v.to_string())); }
        if let Some(v) = active_days { params.push(Box::new(v.to_string())); }
        if let Some(v) = enabled { params.push(Box::new(v as i32)); }
        params.push(Box::new(id));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&query, params_refs.as_slice())?;

        conn.query_row(
            "SELECT id, channel_id, interval_minutes, target, active_hours_start, active_hours_end,
                    active_days, enabled, last_beat_at, next_beat_at, created_at, updated_at
             FROM heartbeat_configs WHERE id = ?1",
            [id],
            |row| self.map_heartbeat_config_row(row),
        )
    }

    /// Update heartbeat last run time
    pub fn update_heartbeat_last_beat(&self, id: i64, last_beat_at: &str, next_beat_at: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE heartbeat_configs SET last_beat_at = ?1, next_beat_at = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![last_beat_at, next_beat_at, now, id],
        )?;

        Ok(())
    }

    /// List all heartbeat configs
    pub fn list_heartbeat_configs(&self) -> SqliteResult<Vec<crate::models::HeartbeatConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, interval_minutes, target, active_hours_start, active_hours_end,
                    active_days, enabled, last_beat_at, next_beat_at, created_at, updated_at
             FROM heartbeat_configs ORDER BY id"
        )?;

        let configs: Vec<crate::models::HeartbeatConfig> = stmt
            .query_map([], |row| self.map_heartbeat_config_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(configs)
    }

    /// Get enabled heartbeat configs that are due to run
    pub fn list_due_heartbeat_configs(&self) -> SqliteResult<Vec<crate::models::HeartbeatConfig>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, channel_id, interval_minutes, target, active_hours_start, active_hours_end,
                    active_days, enabled, last_beat_at, next_beat_at, created_at, updated_at
             FROM heartbeat_configs
             WHERE enabled = 1 AND (next_beat_at IS NULL OR next_beat_at <= ?1)
             ORDER BY next_beat_at ASC"
        )?;

        let configs: Vec<crate::models::HeartbeatConfig> = stmt
            .query_map([&now], |row| self.map_heartbeat_config_row(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(configs)
    }

    // ============================================
    // Gmail Integration methods
    // ============================================

    /// Get Gmail configuration (only one config supported currently)
    pub fn get_gmail_config(&self) -> SqliteResult<Option<crate::integrations::gmail::GmailConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, access_token, refresh_token, token_expires_at,
                    watch_labels, project_id, topic_name, watch_expires_at, history_id,
                    enabled, response_channel_id, auto_reply, created_at, updated_at
             FROM gmail_configs LIMIT 1"
        )?;

        let config = stmt
            .query_row([], |row| self.map_gmail_config_row(row))
            .ok();

        Ok(config)
    }

    /// Get Gmail configuration by email
    pub fn get_gmail_config_by_email(&self, email: &str) -> SqliteResult<Option<crate::integrations::gmail::GmailConfig>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, access_token, refresh_token, token_expires_at,
                    watch_labels, project_id, topic_name, watch_expires_at, history_id,
                    enabled, response_channel_id, auto_reply, created_at, updated_at
             FROM gmail_configs WHERE email = ?1"
        )?;

        let config = stmt
            .query_row([email], |row| self.map_gmail_config_row(row))
            .ok();

        Ok(config)
    }

    /// Create Gmail configuration
    pub fn create_gmail_config(
        &self,
        email: &str,
        access_token: &str,
        refresh_token: &str,
        project_id: &str,
        topic_name: &str,
        watch_labels: &str,
        response_channel_id: Option<i64>,
        auto_reply: bool,
    ) -> SqliteResult<crate::integrations::gmail::GmailConfig> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO gmail_configs (email, access_token, refresh_token, project_id, topic_name,
                                        watch_labels, response_channel_id, auto_reply, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
            rusqlite::params![
                email, access_token, refresh_token, project_id, topic_name,
                watch_labels, response_channel_id, auto_reply as i32, &now
            ],
        )?;

        let id = conn.last_insert_rowid();
        drop(conn);

        self.get_gmail_config().map(|opt| opt.unwrap())
    }

    /// Update Gmail configuration
    pub fn update_gmail_config(
        &self,
        watch_labels: Option<&str>,
        response_channel_id: Option<i64>,
        auto_reply: Option<bool>,
        enabled: Option<bool>,
    ) -> SqliteResult<crate::integrations::gmail::GmailConfig> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Build dynamic update
        let sql = format!(
            "UPDATE gmail_configs SET updated_at = ?1{}{}{}{}",
            watch_labels.map(|_| ", watch_labels = ?2").unwrap_or(""),
            response_channel_id.map(|_| ", response_channel_id = ?3").unwrap_or(""),
            auto_reply.map(|_| ", auto_reply = ?4").unwrap_or(""),
            enabled.map(|_| ", enabled = ?5").unwrap_or(""),
        );

        conn.execute(
            &sql,
            rusqlite::params![
                &now,
                watch_labels.unwrap_or(""),
                response_channel_id.unwrap_or(0),
                auto_reply.unwrap_or(false) as i32,
                enabled.unwrap_or(true) as i32,
            ],
        )?;

        drop(conn);
        self.get_gmail_config().map(|opt| opt.unwrap())
    }

    /// Update Gmail watch info
    pub fn update_gmail_watch(
        &self,
        id: i64,
        watch_expires_at: Option<chrono::DateTime<Utc>>,
        history_id: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let expires_str = watch_expires_at.map(|dt| dt.to_rfc3339());

        conn.execute(
            "UPDATE gmail_configs SET watch_expires_at = ?1, history_id = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![expires_str, history_id, &now, id],
        )?;

        Ok(())
    }

    /// Update Gmail history ID
    pub fn update_gmail_history_id(&self, id: i64, history_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE gmail_configs SET history_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![history_id, &now, id],
        )?;

        Ok(())
    }

    /// Update Gmail tokens
    pub fn update_gmail_tokens(
        &self,
        id: i64,
        access_token: &str,
        token_expires_at: Option<chrono::DateTime<Utc>>,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let expires_str = token_expires_at.map(|dt| dt.to_rfc3339());

        conn.execute(
            "UPDATE gmail_configs SET access_token = ?1, token_expires_at = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![access_token, expires_str, &now, id],
        )?;

        Ok(())
    }

    /// Delete Gmail configuration
    pub fn delete_gmail_config(&self) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute("DELETE FROM gmail_configs", [])?;
        Ok(deleted > 0)
    }

    fn map_gmail_config_row(&self, row: &rusqlite::Row) -> rusqlite::Result<crate::integrations::gmail::GmailConfig> {
        let token_expires_str: Option<String> = row.get(4)?;
        let watch_expires_str: Option<String> = row.get(8)?;
        let created_at_str: String = row.get(13)?;
        let updated_at_str: String = row.get(14)?;

        Ok(crate::integrations::gmail::GmailConfig {
            id: row.get(0)?,
            email: row.get(1)?,
            access_token: row.get(2)?,
            refresh_token: row.get(3)?,
            token_expires_at: token_expires_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            }),
            watch_labels: row.get(5)?,
            project_id: row.get(6)?,
            topic_name: row.get(7)?,
            watch_expires_at: watch_expires_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))
            }),
            history_id: row.get(9)?,
            enabled: row.get::<_, i32>(10)? != 0,
            response_channel_id: row.get(11)?,
            auto_reply: row.get::<_, i32>(12)? != 0,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
        })
    }
}
