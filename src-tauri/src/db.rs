//! Database module for Nkore Browser
//!
//! Manages SQLite persistence for bookmarks, history, and download records.

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: String,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRecord {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub file_size: i64,
    pub downloaded_bytes: i64,
    pub status: String, // "downloading" | "completed" | "failed" | "cancelled"
    pub created_at: String,
    pub completed_at: Option<String>,
    pub mime_type: Option<String>,
}

// ── Database manager ──────────────────────────────────────────────────────────

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens (or creates) the browser SQLite database at the given path.
    pub fn open(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Runs schema migrations.
    fn migrate(&self) -> SqlResult<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS bookmarks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                url         TEXT    NOT NULL UNIQUE,
                title       TEXT    NOT NULL DEFAULT '',
                created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                url         TEXT    NOT NULL,
                title       TEXT    NOT NULL DEFAULT '',
                visited_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                visit_count INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS history_url_idx ON history(url);
            CREATE INDEX IF NOT EXISTS history_visited_idx ON history(visited_at);

            CREATE TABLE IF NOT EXISTS downloads (
                id               TEXT    PRIMARY KEY,
                url              TEXT    NOT NULL,
                filename         TEXT    NOT NULL,
                save_path        TEXT    NOT NULL,
                file_size        INTEGER NOT NULL DEFAULT 0,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                status           TEXT    NOT NULL DEFAULT 'downloading',
                created_at       TEXT    NOT NULL DEFAULT (datetime('now')),
                completed_at     TEXT,
                mime_type        TEXT
            );
            "#,
        )
    }

    // ── Bookmarks ─────────────────────────────────────────────────────────────

    pub fn add_bookmark(&self, url: &str, title: &str) -> SqlResult<Bookmark> {
        self.conn.execute(
            "INSERT OR REPLACE INTO bookmarks (url, title) VALUES (?1, ?2)",
            params![url, title],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_bookmark_by_id(id)
    }

    pub fn remove_bookmark(&self, url: &str) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM bookmarks WHERE url = ?1", params![url])
    }

    pub fn get_bookmarks(&self) -> SqlResult<Vec<Bookmark>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, created_at FROM bookmarks ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn is_bookmarked(&self, url: &str) -> SqlResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE url = ?1",
            params![url],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn get_bookmark_by_id(&self, id: i64) -> SqlResult<Bookmark> {
        self.conn.query_row(
            "SELECT id, url, title, created_at FROM bookmarks WHERE id = ?1",
            params![id],
            |row| {
                Ok(Bookmark {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
    }

    // ── History ───────────────────────────────────────────────────────────────

    /// Inserts or increments a history record for the given URL.
    pub fn add_history(&self, url: &str, title: &str) -> SqlResult<()> {
        // Check if this URL was recently visited (within the last 30 seconds) to
        // avoid duplicate entries on every tiny navigation event.
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM history WHERE url = ?1 AND visited_at > datetime('now', '-30 seconds')",
            params![url],
            |row| row.get(0),
        )?;
        if count > 0 {
            return Ok(());
        }

        // Check if URL exists in history at all
        let existing: SqlResult<i64> = self.conn.query_row(
            "SELECT id FROM history WHERE url = ?1 ORDER BY visited_at DESC LIMIT 1",
            params![url],
            |row| row.get(0),
        );

        match existing {
            Ok(id) => {
                self.conn.execute(
                    "UPDATE history SET visit_count = visit_count + 1, visited_at = datetime('now'), title = ?2 WHERE id = ?1",
                    params![id, title],
                )?;
            }
            Err(_) => {
                self.conn.execute(
                    "INSERT INTO history (url, title) VALUES (?1, ?2)",
                    params![url, title],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_history(&self, limit: i64) -> SqlResult<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visited_at, visit_count FROM history ORDER BY visited_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
                visit_count: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn search_history(&self, query: &str) -> SqlResult<Vec<HistoryEntry>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visited_at, visit_count FROM history WHERE url LIKE ?1 OR title LIKE ?1 ORDER BY visited_at DESC LIMIT 50",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at: row.get(3)?,
                visit_count: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn clear_history(&self) -> SqlResult<usize> {
        self.conn.execute("DELETE FROM history", [])
    }

    // ── Downloads ─────────────────────────────────────────────────────────────

    pub fn insert_download(&self, record: &DownloadRecord) -> SqlResult<()> {
        self.conn.execute(
            r#"INSERT INTO downloads
               (id, url, filename, save_path, file_size, downloaded_bytes, status, created_at, mime_type)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                record.id,
                record.url,
                record.filename,
                record.save_path,
                record.file_size,
                record.downloaded_bytes,
                record.status,
                record.created_at,
                record.mime_type,
            ],
        )?;
        Ok(())
    }

    pub fn update_download_progress(
        &self,
        id: &str,
        downloaded: i64,
        total: i64,
        status: &str,
    ) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE downloads SET downloaded_bytes = ?2, file_size = ?3, status = ?4 WHERE id = ?1",
            params![id, downloaded, total, status],
        )?;
        Ok(())
    }

    pub fn complete_download(&self, id: &str, status: &str) -> SqlResult<()> {
        self.conn.execute(
            "UPDATE downloads SET status = ?2, completed_at = datetime('now') WHERE id = ?1",
            params![id, status],
        )?;
        Ok(())
    }

    pub fn get_downloads(&self) -> SqlResult<Vec<DownloadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, url, filename, save_path, file_size, downloaded_bytes,
                      status, created_at, completed_at, mime_type
               FROM downloads ORDER BY created_at DESC"#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(DownloadRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                filename: row.get(2)?,
                save_path: row.get(3)?,
                file_size: row.get(4)?,
                downloaded_bytes: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
                completed_at: row.get(8)?,
                mime_type: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn clear_completed_downloads(&self) -> SqlResult<usize> {
        self.conn.execute(
            "DELETE FROM downloads WHERE status IN ('completed', 'cancelled', 'failed')",
            [],
        )
    }
}
