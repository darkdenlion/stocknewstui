use crate::model::{Article, Sentiment};
use rusqlite::{params, Connection, Result};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS articles (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL,
                source      TEXT NOT NULL,
                url         TEXT NOT NULL UNIQUE,
                tickers     TEXT NOT NULL DEFAULT '[]',
                published_at INTEGER NOT NULL,
                fetched_at  INTEGER NOT NULL,
                read        INTEGER NOT NULL DEFAULT 0,
                bookmarked  INTEGER NOT NULL DEFAULT 0,
                sentiment   TEXT NOT NULL DEFAULT 'neutral'
            );
            CREATE INDEX IF NOT EXISTS idx_published ON articles(published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_source ON articles(source);
            CREATE INDEX IF NOT EXISTS idx_bookmarked ON articles(bookmarked);",
        )?;

        // Migration: add content column if missing
        let schema: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='articles'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        if !schema.contains("content") {
            let _ = conn.execute_batch("ALTER TABLE articles ADD COLUMN content TEXT DEFAULT NULL;");
        }

        Ok(Db { conn })
    }

    pub fn insert_article(&self, article: &Article) -> Result<bool> {
        let tickers_json = serde_json::to_string(&article.tickers).unwrap_or_default();
        let sentiment_str = match article.sentiment {
            Sentiment::Positive => "positive",
            Sentiment::Negative => "negative",
            Sentiment::Neutral => "neutral",
        };

        let result = self.conn.execute(
            "INSERT OR IGNORE INTO articles (title, source, url, tickers, published_at, fetched_at, sentiment)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                article.title,
                article.source,
                article.url,
                tickers_json,
                article.published_at,
                article.fetched_at,
                sentiment_str,
            ],
        )?;
        Ok(result > 0)
    }

    pub fn get_articles(&self, limit: usize) -> Result<Vec<Article>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, source, url, tickers, published_at, fetched_at, read, bookmarked, sentiment
             FROM articles ORDER BY published_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tickers_str: String = row.get(4)?;
            let tickers: Vec<String> =
                serde_json::from_str(&tickers_str).unwrap_or_default();
            let sentiment_str: String = row.get(9)?;
            let sentiment = match sentiment_str.as_str() {
                "positive" => Sentiment::Positive,
                "negative" => Sentiment::Negative,
                _ => Sentiment::Neutral,
            };
            Ok(Article {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                url: row.get(3)?,
                tickers,
                published_at: row.get(5)?,
                fetched_at: row.get(6)?,
                read: row.get::<_, i32>(7)? != 0,
                bookmarked: row.get::<_, i32>(8)? != 0,
                sentiment,
            })
        })?;

        rows.collect()
    }

    pub fn get_articles_by_tickers(&self, tickers: &[String], limit: usize) -> Result<Vec<Article>> {
        if tickers.is_empty() {
            return self.get_articles(limit);
        }

        // Build LIKE conditions for each ticker
        let conditions: Vec<String> = tickers
            .iter()
            .map(|t| format!("(tickers LIKE '%\"{}%' OR UPPER(title) LIKE '%{}%')", t, t))
            .collect();
        let where_clause = conditions.join(" OR ");

        let query = format!(
            "SELECT id, title, source, url, tickers, published_at, fetched_at, read, bookmarked, sentiment
             FROM articles WHERE {} ORDER BY published_at DESC LIMIT ?1",
            where_clause
        );

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let tickers_str: String = row.get(4)?;
            let article_tickers: Vec<String> =
                serde_json::from_str(&tickers_str).unwrap_or_default();
            let sentiment_str: String = row.get(9)?;
            let sentiment = match sentiment_str.as_str() {
                "positive" => Sentiment::Positive,
                "negative" => Sentiment::Negative,
                _ => Sentiment::Neutral,
            };
            Ok(Article {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                url: row.get(3)?,
                tickers: article_tickers,
                published_at: row.get(5)?,
                fetched_at: row.get(6)?,
                read: row.get::<_, i32>(7)? != 0,
                bookmarked: row.get::<_, i32>(8)? != 0,
                sentiment,
            })
        })?;

        rows.collect()
    }

    pub fn get_unread_articles(&self, limit: usize) -> Result<Vec<Article>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, source, url, tickers, published_at, fetched_at, read, bookmarked, sentiment
             FROM articles WHERE read = 0 ORDER BY published_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tickers_str: String = row.get(4)?;
            let tickers: Vec<String> =
                serde_json::from_str(&tickers_str).unwrap_or_default();
            let sentiment_str: String = row.get(9)?;
            let sentiment = match sentiment_str.as_str() {
                "positive" => Sentiment::Positive,
                "negative" => Sentiment::Negative,
                _ => Sentiment::Neutral,
            };
            Ok(Article {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                url: row.get(3)?,
                tickers,
                published_at: row.get(5)?,
                fetched_at: row.get(6)?,
                read: row.get::<_, i32>(7)? != 0,
                bookmarked: row.get::<_, i32>(8)? != 0,
                sentiment,
            })
        })?;

        rows.collect()
    }

    pub fn get_bookmarked_articles(&self, limit: usize) -> Result<Vec<Article>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, source, url, tickers, published_at, fetched_at, read, bookmarked, sentiment
             FROM articles WHERE bookmarked = 1 ORDER BY published_at DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tickers_str: String = row.get(4)?;
            let tickers: Vec<String> =
                serde_json::from_str(&tickers_str).unwrap_or_default();
            let sentiment_str: String = row.get(9)?;
            let sentiment = match sentiment_str.as_str() {
                "positive" => Sentiment::Positive,
                "negative" => Sentiment::Negative,
                _ => Sentiment::Neutral,
            };
            Ok(Article {
                id: row.get(0)?,
                title: row.get(1)?,
                source: row.get(2)?,
                url: row.get(3)?,
                tickers,
                published_at: row.get(5)?,
                fetched_at: row.get(6)?,
                read: row.get::<_, i32>(7)? != 0,
                bookmarked: row.get::<_, i32>(8)? != 0,
                sentiment,
            })
        })?;

        rows.collect()
    }

    pub fn mark_read(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE articles SET read = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_bookmark(&self, id: i64) -> Result<bool> {
        self.conn.execute(
            "UPDATE articles SET bookmarked = CASE WHEN bookmarked = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![id],
        )?;

        let bookmarked: bool = self.conn.query_row(
            "SELECT bookmarked FROM articles WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(bookmarked)
    }

    pub fn article_count(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))
    }

    pub fn unread_count(&self) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM articles WHERE read = 0",
            [],
            |row| row.get(0),
        )
    }

    pub fn save_content(&self, article_id: i64, content: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE articles SET content = ?1 WHERE id = ?2",
            params![content, article_id],
        )?;
        Ok(())
    }

    pub fn get_content(&self, article_id: i64) -> Result<Option<String>> {
        self.conn.query_row(
            "SELECT content FROM articles WHERE id = ?1",
            params![article_id],
            |row| row.get(0),
        )
    }
}
