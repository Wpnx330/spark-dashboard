use rusqlite::{params, Connection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Thread-safe handle to the history database.
#[derive(Clone)]
pub struct HistoryDb {
    inner: Arc<Mutex<Connection>>,
    /// Whether the user has opted in to historical logging (atomic for fast reads).
    enabled: Arc<AtomicBool>,
}

impl HistoryDb {
    /// Open (or create) the database and run migrations.
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Self::migrate(&conn)?;
        let enabled = Arc::new(AtomicBool::new(false));
        // Read current setting from DB
        if let Ok(Some(val)) = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'history_enabled'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map(Some)
            .or::<rusqlite::Error>(Ok(None))
        {
            enabled.store(val == "true", Ordering::Relaxed);
        }
        Ok(HistoryDb {
            inner: Arc::new(Mutex::new(conn)),
            enabled,
        })
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS snapshots_1s (
                engine_key         TEXT NOT NULL,
                ts                 INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens   INTEGER,
                total_requests     INTEGER,
                prompt_tps         REAL,
                decode_tps         REAL,
                ttft_ms            REAL,
                itl_ms             REAL,
                e2e_ms             REAL,
                power_watts        REAL,
                gpu_util           REAL,
                gpu_temp           REAL,
                active_requests    INTEGER,
                queued_requests    INTEGER,
                kv_cache_pct       REAL,
                prefix_cache_hit   REAL,
                cpu_util           REAL,
                mem_used_pct       REAL
            );
            CREATE INDEX IF NOT EXISTS idx_1s_engine_ts ON snapshots_1s(engine_key, ts);

            CREATE TABLE IF NOT EXISTS snapshots_1h (
                engine_key          TEXT NOT NULL,
                bucket_ts           INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens    INTEGER,
                total_requests      INTEGER,
                prompt_tps_avg      REAL,
                prompt_tps_max      REAL,
                decode_tps_avg      REAL,
                decode_tps_max      REAL,
                ttft_ms_p95         REAL,
                itl_ms_p95          REAL,
                e2e_ms_p95          REAL,
                power_watts_sum     REAL,
                gpu_util_avg        REAL,
                gpu_temp_avg        REAL,
                gpu_temp_max        REAL,
                active_requests_max INTEGER,
                queued_requests_max INTEGER,
                kv_cache_pct_avg    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                UNIQUE(engine_key, bucket_ts)
            );
            CREATE INDEX IF NOT EXISTS idx_1h_engine_ts ON snapshots_1h(engine_key, bucket_ts);

            CREATE TABLE IF NOT EXISTS snapshots_1d (
                engine_key          TEXT NOT NULL,
                bucket_ts           INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens    INTEGER,
                total_requests      INTEGER,
                prompt_tps_avg      REAL,
                prompt_tps_max      REAL,
                decode_tps_avg      REAL,
                decode_tps_max      REAL,
                ttft_ms_p95         REAL,
                itl_ms_p95          REAL,
                e2e_ms_p95          REAL,
                power_watts_sum     REAL,
                gpu_util_avg        REAL,
                gpu_temp_avg        REAL,
                gpu_temp_max        REAL,
                active_requests_max INTEGER,
                queued_requests_max INTEGER,
                kv_cache_pct_avg    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_1d_engine_ts ON snapshots_1d(engine_key, bucket_ts);
        ",
        )?;
        // Clear old cumulative data (now using deltas)
        conn.execute(
            "DELETE FROM snapshots_1s WHERE COALESCE(total_prompt_tokens,0) > 10000000000000",
            [],
        )?;
        conn.execute(
            "DELETE FROM snapshots_1h WHERE COALESCE(total_prompt_tokens,0) > 10000000000000",
            [],
        )?;
        conn.execute(
            "DELETE FROM snapshots_1d WHERE COALESCE(total_prompt_tokens,0) > 10000000000000",
            [],
        )?;
        Ok(())
    }

    /// Check if historical logging is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Toggle historical logging on/off and persist the setting.
    pub async fn set_enabled(&self, on: bool) -> rusqlite::Result<()> {
        let val = if on { "true" } else { "false" };
        self.enabled.store(on, Ordering::Relaxed);
        let db = self.inner.lock().await;
        db.execute(
            "INSERT INTO settings (key, value) VALUES ('history_enabled', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![val],
        )?;
        info!(
            "History logging {}",
            if on { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Get a setting value by key.
    pub async fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        let db = self.inner.lock().await;
        db.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
    }

    /// Upsert a setting value.
    pub async fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        let db = self.inner.lock().await;
        db.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Insert a 1-second snapshot. Called every poll cycle.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_1s(
        &self,
        engine_key: &str,
        ts: i64,
        total_prompt_tokens: Option<i64>,
        total_gen_tokens: Option<i64>,
        total_requests: Option<i64>,
        prompt_tps: Option<f64>,
        decode_tps: Option<f64>,
        ttft_ms: Option<f64>,
        itl_ms: Option<f64>,
        e2e_ms: Option<f64>,
        power_watts: Option<f64>,
        gpu_util: Option<f64>,
        gpu_temp: Option<f64>,
        active_requests: Option<i64>,
        queued_requests: Option<i64>,
        kv_cache_pct: Option<f64>,
        prefix_cache_hit: Option<f64>,
        cpu_util: Option<f64>,
        mem_used_pct: Option<f64>,
    ) -> rusqlite::Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }
        let db = self.inner.lock().await;
        db.execute(
            "INSERT INTO snapshots_1s
             (engine_key, ts, total_prompt_tokens, total_gen_tokens, total_requests,
              prompt_tps, decode_tps, ttft_ms, itl_ms, e2e_ms,
              power_watts, gpu_util, gpu_temp, active_requests, queued_requests,
              kv_cache_pct, prefix_cache_hit, cpu_util, mem_used_pct)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![
                engine_key,
                ts,
                total_prompt_tokens,
                total_gen_tokens,
                total_requests,
                prompt_tps,
                decode_tps,
                ttft_ms,
                itl_ms,
                e2e_ms,
                power_watts,
                gpu_util,
                gpu_temp,
                active_requests,
                queued_requests,
                kv_cache_pct,
                prefix_cache_hit,
                cpu_util,
                mem_used_pct,
            ],
        )?;
        Ok(())
    }

    /// Roll up completed hours into hourly records and prune the 1s source data.
    /// Should be called periodically (e.g. once per hour by a background task).
    pub async fn rollup_1s_to_1h(&self) -> rusqlite::Result<u64> {
        let db = self.inner.lock().await;
        // Find all complete hours that haven't been rolled up yet.
        // We compute the latest hour boundary from the data.
        // A "complete hour" is one whose all-60-minutes-worth of data has
        // passed — i.e., the current time has moved past that hour.
        let now_ms = chrono_now_ms();
        let current_hour_start = (now_ms / 3_600_000) * 3_600_000;

        // Roll up every complete hour that has data
        let rows = db.execute(
            "INSERT OR IGNORE INTO snapshots_1h
             (engine_key, bucket_ts,
              total_prompt_tokens, total_gen_tokens, total_requests,
              prompt_tps_avg, prompt_tps_max,
              decode_tps_avg, decode_tps_max,
              power_watts_sum,
              gpu_util_avg, gpu_temp_avg, gpu_temp_max,
              active_requests_max, queued_requests_max,
              kv_cache_pct_avg, prefix_cache_hit_avg,
              cpu_util_avg, sample_count)
             SELECT
               engine_key, (ts / 3600000) * 3600000,
               SUM(COALESCE(total_prompt_tokens,0)), SUM(COALESCE(total_gen_tokens,0)),
               SUM(COALESCE(total_requests,0)),
               AVG(prompt_tps), MAX(prompt_tps),
               AVG(decode_tps), MAX(decode_tps),
               SUM(power_watts),
               AVG(gpu_util), AVG(gpu_temp), MAX(gpu_temp),
               MAX(active_requests), MAX(queued_requests),
               AVG(kv_cache_pct), AVG(prefix_cache_hit),
               AVG(cpu_util), COUNT(*)
             FROM snapshots_1s
             WHERE ts < ?1
             GROUP BY engine_key, (ts / 3600000)
             ON CONFLICT(engine_key, bucket_ts) DO NOTHING",
            params![current_hour_start],
        )?;

        // Prune the 1s data that was just rolled up (any completed hour)
        let deleted = db.execute(
            "DELETE FROM snapshots_1s WHERE ts < ?1",
            params![current_hour_start],
        )?;

        if rows > 0 {
            info!(
                "History: rolled up {} hours from 1s data, pruned {} rows",
                rows, deleted
            );
        }
        Ok(rows as u64)
    }

    /// Roll up completed days from hourly data.
    pub async fn rollup_1h_to_1d(&self) -> rusqlite::Result<u64> {
        let db = self.inner.lock().await;
        let now_ms = chrono_now_ms();
        let current_day_start = (now_ms / 86_400_000) * 86_400_000;

        let rows = db.execute(
            "INSERT OR IGNORE INTO snapshots_1d
             (engine_key, bucket_ts,
              total_prompt_tokens, total_gen_tokens, total_requests,
              prompt_tps_avg, prompt_tps_max,
              decode_tps_avg, decode_tps_max,
              power_watts_sum,
              gpu_util_avg, gpu_temp_avg, gpu_temp_max,
              active_requests_max, queued_requests_max,
              kv_cache_pct_avg, prefix_cache_hit_avg,
              cpu_util_avg, sample_count)
             SELECT
               engine_key, (bucket_ts / 86400000) * 86400000,
               SUM(COALESCE(total_prompt_tokens,0)), SUM(COALESCE(total_gen_tokens,0)),
               SUM(COALESCE(total_requests,0)),
               AVG(prompt_tps_avg), MAX(prompt_tps_max),
               AVG(decode_tps_avg), MAX(decode_tps_max),
               SUM(power_watts_sum),
               AVG(gpu_util_avg), AVG(gpu_temp_avg), MAX(gpu_temp_max),
               MAX(active_requests_max), MAX(queued_requests_max),
               AVG(kv_cache_pct_avg), AVG(prefix_cache_hit_avg),
               AVG(cpu_util_avg), SUM(sample_count)
             FROM snapshots_1h
             WHERE bucket_ts < ?1
             GROUP BY engine_key, (bucket_ts / 86400000)
             ON CONFLICT(engine_key, bucket_ts) DO NOTHING",
            params![current_day_start],
        )?;

        // Prune hourly data older than 30 days
        let cutoff = now_ms - 30 * 86_400_000;
        let deleted = db.execute(
            "DELETE FROM snapshots_1h WHERE bucket_ts < ?1",
            params![cutoff],
        )?;

        if rows > 0 {
            info!(
                "History: rolled up {} days from hourly data, pruned {} stale hourly rows",
                rows, deleted
            );
        }
        Ok(rows as u64)
    }

    /// Query summary stats for a given engine and time window.
    /// Returns (delta_prompt, delta_gen, avg_decode_tps, avg_prompt_tps,
    ///          peak_active, peak_queued, power_kwh, total_seconds)
    pub async fn query_summary(
        &self,
        engine_key: &str,
        since_ms: i64,
        until_ms: i64,
    ) -> rusqlite::Result<Option<HistorySummary>> {
        let db = self.inner.lock().await;

        // All three queries use the same structure: SUM for deltas, MAX for gauges.
        // Try daily → hourly → raw, falling through if empty.
        let try_tables = [
            ("snapshots_1d", "daily", "bucket_ts"),
            ("snapshots_1h", "hourly", "bucket_ts"),
            ("snapshots_1s", "raw", "ts"),
        ];

        for (table, source, ts_col) in &try_tables {
            // For raw data: each row = 1 second of data.
            // For rolled-up data: use sample_count (stored in 1h/1d tables) to weight
            // averages and compute actual runtime instead of calendar span.
            let gauge_suffix = if *source == "raw" { "" } else { "_max" };
            let power_suffix = if *source == "raw" { "" } else { "_sum" };
            let sql = if *source == "raw" {
                format!(
                    "SELECT
                       COALESCE(SUM(total_prompt_tokens),0),
                       COALESCE(SUM(total_gen_tokens),0),
                       COALESCE(SUM(total_requests), 0),
                       AVG(decode_tps),
                       AVG(prompt_tps),
                       MAX(active_requests),
                       MAX(queued_requests),
                       SUM(power_watts),
                       COUNT(*)
                     FROM {}
                      WHERE engine_key = ?1 AND {} >= ?2 AND {} <= ?3",
                    table, ts_col, ts_col,
                )
            } else {
                // Weighted average: SUM(val_avg * sample_count) / SUM(sample_count)
                // Runtime: SUM(sample_count) seconds (each sample = 1 second of raw data)
                format!(
                     "SELECT
                        COALESCE(SUM(total_prompt_tokens),0),
                        COALESCE(SUM(total_gen_tokens),0),
                        COALESCE(SUM(total_requests), 0),
                        COALESCE(SUM(decode_tps_avg * sample_count) / NULLIF(SUM(sample_count), 0), 0),
                        COALESCE(SUM(prompt_tps_avg * sample_count) / NULLIF(SUM(sample_count), 0), 0),
                        MAX(active_requests{}),
                        MAX(queued_requests{}),
                        SUM(power_watts{}),
                        SUM(sample_count)
                      FROM {}
                      WHERE engine_key = ?1 AND {} >= ?2 AND {} <= ?3",
                     gauge_suffix, gauge_suffix, power_suffix, table, ts_col, ts_col,
                 )
            };

            let result = db.query_row(&sql, params![engine_key, since_ms, until_ms], |r| {
                let delta_prompt: i64 = r.get::<_, Option<i64>>(0)?.unwrap_or(0);
                let delta_gen: i64 = r.get::<_, Option<i64>>(1)?.unwrap_or(0);
                let total_reqs: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
                let avg_decode: f64 = r.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
                let avg_prompt: f64 = r.get::<_, Option<f64>>(4)?.unwrap_or(0.0);
                let peak_active: i64 = r.get::<_, Option<i64>>(5)?.unwrap_or(0);
                let peak_queued: i64 = r.get::<_, Option<i64>>(6)?.unwrap_or(0);
                let power_sum: f64 = r.get::<_, Option<f64>>(7)?.unwrap_or(0.0);
                let count: i64 = r.get::<_, Option<i64>>(8)?.unwrap_or(0);
                Ok(HistorySummary {
                    delta_prompt_tokens: delta_prompt,
                    delta_gen_tokens: delta_gen,
                    total_requests: total_reqs,
                    avg_decode_tps: avg_decode,
                    avg_prompt_tps: avg_prompt,
                    peak_active_requests: peak_active,
                    peak_queued_requests: peak_queued,
                    power_kwh: power_sum / 3600.0 / 1000.0,
                    total_seconds: Some(count as f64),
                    source_table: source,
                })
            });

            match result {
                Ok(summary) => {
                    // Only return if there were actually non-zero values
                    if summary.delta_prompt_tokens > 0
                        || summary.delta_gen_tokens > 0
                        || summary.total_requests > 0
                    {
                        return Ok(Some(summary));
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => continue,
                Err(e) => {
                    tracing::warn!("History query failed on {}: {}", table, e);
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// Get database size in bytes.
    pub async fn db_size(&self) -> rusqlite::Result<i64> {
        let db = self.inner.lock().await;
        db.query_row("SELECT COALESCE(SUM(pgsize), 0) FROM dbstat", [], |r| {
            r.get(0)
        })
    }

    /// Prune data older than the given timestamp across all tables.
    pub async fn prune(&self, older_than_ms: i64) -> rusqlite::Result<(usize, usize, usize)> {
        let db = self.inner.lock().await;
        let s1 = db.execute(
            "DELETE FROM snapshots_1s WHERE ts < ?1",
            params![older_than_ms],
        )?;
        let h1 = db.execute(
            "DELETE FROM snapshots_1h WHERE bucket_ts < ?1",
            params![older_than_ms],
        )?;
        let d1 = db.execute(
            "DELETE FROM snapshots_1d WHERE bucket_ts < ?1",
            params![older_than_ms],
        )?;
        info!(
            "History: pruned {}s+{}h+{}d rows older than ts={}",
            s1, h1, d1, older_than_ms
        );
        Ok((s1, h1, d1))
    }
}

/// Summary statistics returned by the history query endpoint.
#[derive(serde::Serialize, Clone, Debug)]
pub struct HistorySummary {
    pub delta_prompt_tokens: i64,
    pub delta_gen_tokens: i64,
    pub avg_decode_tps: f64,
    pub avg_prompt_tps: f64,
    pub peak_active_requests: i64,
    pub peak_queued_requests: i64,
    pub total_requests: i64,
    /// Total energy consumption in kilowatt-hours.
    pub power_kwh: f64,
    /// Total seconds represented by the data (for calculating hours alive).
    pub total_seconds: Option<f64>,
    /// Which internal table satisfied the query (raw | hourly | daily).
    pub source_table: &'static str,
}

/// Current Unix timestamp in milliseconds.
fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Open an in-memory database for testing (skipping file I/O).
    fn test_db() -> HistoryDb {
        // We bypass HistoryDb::open and manually construct from an in-memory conn.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .unwrap();
        // Run migrations manually since we aren't using HistoryDb::open
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS snapshots_1s (
                engine_key         TEXT NOT NULL,
                ts                 INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens   INTEGER,
                total_requests     INTEGER,
                prompt_tps         REAL,
                decode_tps         REAL,
                ttft_ms            REAL,
                itl_ms             REAL,
                e2e_ms             REAL,
                power_watts        REAL,
                gpu_util           REAL,
                gpu_temp           REAL,
                active_requests    INTEGER,
                queued_requests    INTEGER,
                kv_cache_pct       REAL,
                prefix_cache_hit   REAL,
                cpu_util           REAL,
                mem_used_pct       REAL
            );
            CREATE TABLE IF NOT EXISTS snapshots_1h (
                engine_key          TEXT NOT NULL,
                bucket_ts           INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens    INTEGER,
                total_requests      INTEGER,
                prompt_tps_avg      REAL,
                prompt_tps_max      REAL,
                decode_tps_avg      REAL,
                decode_tps_max      REAL,
                ttft_ms_p95         REAL,
                itl_ms_p95          REAL,
                e2e_ms_p95          REAL,
                power_watts_sum     REAL,
                gpu_util_avg        REAL,
                gpu_temp_avg        REAL,
                gpu_temp_max        REAL,
                active_requests_max INTEGER,
                queued_requests_max INTEGER,
                kv_cache_pct_avg    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                UNIQUE(engine_key, bucket_ts)
            );
            CREATE TABLE IF NOT EXISTS snapshots_1d (
                engine_key          TEXT NOT NULL,
                bucket_ts           INTEGER NOT NULL,
                total_prompt_tokens INTEGER,
                total_gen_tokens    INTEGER,
                total_requests      INTEGER,
                prompt_tps_avg      REAL,
                prompt_tps_max      REAL,
                decode_tps_avg      REAL,
                decode_tps_max      REAL,
                ttft_ms_p95         REAL,
                itl_ms_p95          REAL,
                e2e_ms_p95          REAL,
                power_watts_sum     REAL,
                gpu_util_avg        REAL,
                gpu_temp_avg        REAL,
                gpu_temp_max        REAL,
                active_requests_max INTEGER,
                queued_requests_max INTEGER,
                kv_cache_pct_avg    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL
            );
        ",
        )
        .unwrap();
        let enabled = Arc::new(AtomicBool::new(true));
        HistoryDb {
            inner: Arc::new(Mutex::new(conn)),
            enabled,
        }
    }

    #[tokio::test]
    async fn test_insert_and_query_1s() {
        let db = test_db();
        let key = "test-engine";
        let now = chrono_now_ms();

        db.insert_1s(
            key,
            now,
            Some(100),
            Some(200),
            Some(5),
            Some(50.0),
            Some(30.0),
            Some(10.0),
            Some(5.0),
            Some(100.0),
            Some(150.0),
            Some(80.0),
            Some(45.0),
            Some(3),
            Some(1),
            Some(0.75),
            Some(0.1),
            Some(60.0),
            Some(50.0),
        )
        .await
        .unwrap();

        let summary = db.query_summary(key, now - 1000, now + 1000).await.unwrap();
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(s.delta_prompt_tokens, 100);
        assert_eq!(s.delta_gen_tokens, 200);
        assert_eq!(s.total_requests, 5);
        assert_eq!(s.source_table, "raw");
        assert!(s.power_kwh > 0.0);
    }

    #[tokio::test]
    async fn test_setting_persistence() {
        let db = test_db();
        db.set_setting("cloud_prompt_rate", "1.50").await.unwrap();
        let val = db.get_setting("cloud_prompt_rate").await.unwrap();
        assert_eq!(val, Some("1.50".to_string()));
    }

    #[tokio::test]
    async fn test_is_enabled_defaults_true() {
        let db = test_db();
        assert!(db.is_enabled());
    }

    #[tokio::test]
    async fn test_toggle_enabled() {
        let db = test_db();
        db.set_enabled(false).await.unwrap();
        assert!(!db.is_enabled());
        db.set_enabled(true).await.unwrap();
        assert!(db.is_enabled());
    }

    #[tokio::test]
    async fn test_insert_disabled_does_nothing() {
        let db = test_db();
        db.set_enabled(false).await.unwrap();
        let key = "test-engine";
        let now = chrono_now_ms();
        db.insert_1s(
            key,
            now,
            Some(100),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let summary = db.query_summary(key, now - 1000, now + 1000).await.unwrap();
        assert!(summary.is_none());
    }

    #[tokio::test]
    async fn test_rollup_1s_to_1h() {
        let db = test_db();
        let key = "test-engine";
        // Insert data across two different hours
        let hour1 = (chrono_now_ms() / 3_600_000) * 3_600_000 - 7_200_000; // 2 hours ago
        let hour2 = (chrono_now_ms() / 3_600_000) * 3_600_000 - 3_600_000; // 1 hour ago

        db.insert_1s(
            key,
            hour1 + 1000,
            Some(50),
            Some(100),
            Some(2),
            Some(40.0),
            Some(25.0),
            None,
            None,
            None,
            Some(100.0),
            Some(70.0),
            Some(40.0),
            Some(2),
            Some(0),
            Some(0.5),
            Some(0.05),
            Some(55.0),
            Some(45.0),
        )
        .await
        .unwrap();
        db.insert_1s(
            key,
            hour1 + 2000,
            Some(60),
            Some(120),
            Some(3),
            Some(45.0),
            Some(28.0),
            None,
            None,
            None,
            Some(110.0),
            Some(72.0),
            Some(41.0),
            Some(3),
            Some(1),
            Some(0.6),
            Some(0.06),
            Some(58.0),
            Some(48.0),
        )
        .await
        .unwrap();
        db.insert_1s(
            key,
            hour2 + 1000,
            Some(70),
            Some(140),
            Some(4),
            Some(50.0),
            Some(30.0),
            None,
            None,
            None,
            Some(120.0),
            Some(75.0),
            Some(42.0),
            Some(4),
            Some(0),
            Some(0.55),
            Some(0.04),
            Some(60.0),
            Some(50.0),
        )
        .await
        .unwrap();

        let rolled = db.rollup_1s_to_1h().await.unwrap();
        assert_eq!(rolled, 2, "should roll up 2 hourly buckets");

        // Query the hourly data — it should fall through to hourly table
        let summary = db.query_summary(key, hour1, hour2 + 5000).await.unwrap();
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(s.source_table, "hourly");
        assert_eq!(s.total_requests, 9, "should sum all requests: 2+3+4");
    }

    #[tokio::test]
    async fn test_query_no_data_returns_none() {
        let db = test_db();
        let summary = db
            .query_summary("nonexistent", 0, chrono_now_ms())
            .await
            .unwrap();
        assert!(summary.is_none());
    }

    #[tokio::test]
    async fn test_prune_removes_old_data() {
        let db = test_db();
        let key = "test-engine";
        let now = chrono_now_ms();

        db.insert_1s(
            key,
            now - 100_000,
            Some(10),
            Some(20),
            Some(1),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        db.insert_1s(
            key,
            now - 50_000,
            Some(30),
            Some(40),
            Some(2),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Prune data older than 60 seconds ago
        let (s, _, _) = db.prune(now - 60_000).await.unwrap();
        assert_eq!(s, 1, "should delete 1 old row");

        let summary = db.query_summary(key, now - 200_000, now).await.unwrap();
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(
            s.delta_prompt_tokens, 30,
            "only the newer row should remain"
        );
    }
}
