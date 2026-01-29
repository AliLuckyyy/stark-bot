//! SQLite database - schema definitions and connection management
//!
//! This file contains:
//! - Database struct definition
//! - Connection management (new, init)
//! - Schema creation and migrations
//!
//! All database operations are in the models/ subdirectory.

use rusqlite::{Connection, Result as SqliteResult};
use std::path::Path;
use std::sync::Mutex;

/// Main database wrapper with connection pooling via Mutex
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    /// Create a new database connection and initialize schema
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

    /// Initialize all database tables and run migrations
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
                public_address TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            )",
            [],
        )?;

        // Auth challenges table for SIWE
        conn.execute(
            "CREATE TABLE IF NOT EXISTS auth_challenges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                public_address TEXT UNIQUE NOT NULL,
                challenge TEXT NOT NULL,
                created_at TEXT NOT NULL
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

        // Agent settings table (AI endpoint configuration - simplified for x402)
        // Note: provider, api_key, model columns are deprecated (kept for migration compatibility)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                endpoint TEXT NOT NULL,
                model_archetype TEXT NOT NULL DEFAULT 'kimi',
                max_tokens INTEGER NOT NULL DEFAULT 40000,
                enabled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Bot settings table (git commit author info, etc.)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bot_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bot_name TEXT NOT NULL DEFAULT 'StarkBot',
                bot_email TEXT NOT NULL DEFAULT 'starkbot@users.noreply.github.com',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Migration: Add model_archetype column if it doesn't exist (for old DBs)
        let has_model_archetype: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agent_settings') WHERE name='model_archetype'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_model_archetype {
            conn.execute("ALTER TABLE agent_settings ADD COLUMN model_archetype TEXT DEFAULT 'kimi'", [])?;
        }

        // Migration: Add max_tokens column if it doesn't exist (for old DBs)
        let has_max_tokens: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agent_settings') WHERE name='max_tokens'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        if !has_max_tokens {
            conn.execute("ALTER TABLE agent_settings ADD COLUMN max_tokens INTEGER DEFAULT 40000", [])?;
        }

        // Initialize bot_settings with defaults if empty
        let bot_settings_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bot_settings", [], |row| row.get(0))
            .unwrap_or(0);

        if bot_settings_count == 0 {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO bot_settings (bot_name, bot_email, created_at, updated_at) VALUES ('StarkBot', 'starkbot@users.noreply.github.com', ?1, ?2)",
                [&now, &now],
            )?;
        }

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

            //this is an anti pattern ! 
        // Seed default Kimi agent if no agents exist
       /* let agent_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_settings",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if agent_count == 0 {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO agent_settings (provider, endpoint, api_key, model, model_archetype, max_tokens, enabled, created_at, updated_at)
                 VALUES ('kimi', 'https://kimi.defirelay.com/api/v1/chat/completions', '', 'default', 'kimi', 40000, 1, ?1, ?2)",
                [&now, &now],
            )?;
            log::info!("Seeded default Kimi agent");
        }*/

        Ok(())
    }
}
