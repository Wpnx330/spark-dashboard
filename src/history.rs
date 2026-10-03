use rusqlite::{params, Connection};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// How many recent days the 1h→1d rollup re-aggregates on each tick: the
/// current (partial) day plus the previous `DAILY_ROLLUP_LOOKBACK_DAYS - 1`
/// completed days. Days older than this window are NEVER touched by the
/// rollup.
///
/// WHY bounded: the same rollup prunes hourly rows older than 30 days, so
/// re-aggregating an old day from its (partially pruned) hourly sources
/// OVERWRITES the stored daily totals with a shrinking remainder — the cause
/// of the ever-decreasing "All time" cost-avoided total. Once a day's last
/// hourly row is pruned, the daily row would be frozen at whatever the last
/// partial aggregation produced (e.g. sample_count=3600 instead of 86400).
const DAILY_ROLLUP_LOOKBACK_DAYS: i64 = 3;

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

    pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
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
                mem_used_pct       REAL,
                preemptions_total  INTEGER,
                queue_time_ms      REAL,
                tpot_ms            REAL
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
                kv_cache_pct_max    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                preemptions_total   INTEGER,
                queue_time_ms_avg   REAL,
                tpot_ms_avg         REAL,
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
                kv_cache_pct_max    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                preemptions_total   INTEGER,
                queue_time_ms_avg   REAL,
                tpot_ms_avg         REAL
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

        // Add columns if they don't exist (SQLite has no IF NOT EXISTS for
        // ALTER TABLE ADD COLUMN). Each check is guarded by PRAGMA table_info
        // so existing databases are migrated without losing data.
        let add_col =
            |conn: &Connection, table: &str, col: &str, decl: &str| -> rusqlite::Result<()> {
                let cols: Vec<String> = conn
                    .prepare(&format!("PRAGMA table_info({})", table))?
                    .query_map([], |r| r.get::<_, String>(1))?
                    .filter_map(Result::ok)
                    .collect();
                if !cols.contains(&col.to_string()) {
                    conn.execute_batch(&format!(
                        "ALTER TABLE {} ADD COLUMN {} {}",
                        table, col, decl
                    ))?;
                }
                Ok(())
            };
        add_col(conn, "snapshots_1s", "preemptions_total", "INTEGER")?;
        add_col(conn, "snapshots_1s", "queue_time_ms", "REAL")?;
        add_col(conn, "snapshots_1s", "tpot_ms", "REAL")?;
        add_col(conn, "snapshots_1h", "kv_cache_pct_max", "REAL")?;
        add_col(conn, "snapshots_1h", "preemptions_total", "INTEGER")?;
        add_col(conn, "snapshots_1h", "queue_time_ms_avg", "REAL")?;
        add_col(conn, "snapshots_1h", "tpot_ms_avg", "REAL")?;
        add_col(conn, "snapshots_1d", "kv_cache_pct_max", "REAL")?;
        add_col(conn, "snapshots_1d", "preemptions_total", "INTEGER")?;
        add_col(conn, "snapshots_1d", "queue_time_ms_avg", "REAL")?;
        add_col(conn, "snapshots_1d", "tpot_ms_avg", "REAL")?;
        // TAR (Token Acceptance Rate) — added for Cache chart overlay.
        add_col(conn, "snapshots_1s", "spec_decode_acceptance_rate", "REAL")?;
        add_col(
            conn,
            "snapshots_1h",
            "spec_decode_acceptance_rate_avg",
            "REAL",
        )?;
        add_col(
            conn,
            "snapshots_1d",
            "spec_decode_acceptance_rate_avg",
            "REAL",
        )?;

        // Legacy databases may have been created before the UNIQUE constraint
        // on (engine_key, bucket_ts) was added to the CREATE TABLE statement.
        // The rollup INSERT … ON CONFLICT(engine_key, bucket_ts) requires a
        // UNIQUE constraint or PRIMARY KEY covering those columns. Add a
        // unique index if the table itself doesn't have one (IF NOT EXISTS
        // makes this idempotent).
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_1h_unique ON snapshots_1h(engine_key, bucket_ts);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_1d_unique ON snapshots_1d(engine_key, bucket_ts);",
        )?;

        // One-time backfill: historical `power_watts` values were recorded
        // from a single node's GPU(s) only. Now that multi-node monitoring is
        // active, multiply every existing power value by the cluster size (4
        // DGX nodes) so old data is comparable to the new total-cluster
        // recordings. Guarded by the `power_backfill_done` settings key so it
        // runs exactly once.
        const CLUSTER_NODE_COUNT: f64 = 4.0;
        let backfill_done: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'power_backfill_done'",
                [],
                |r| r.get::<_, String>(0),
            )
            .map(Some)
            .or::<rusqlite::Error>(Ok(None))?;
        if backfill_done.is_none() {
            let updated_1s = conn.execute(
                "UPDATE snapshots_1s SET power_watts = power_watts * ?1 \
                 WHERE power_watts IS NOT NULL",
                params![CLUSTER_NODE_COUNT],
            )?;
            let updated_1h = conn.execute(
                "UPDATE snapshots_1h SET power_watts_sum = power_watts_sum * ?1 \
                 WHERE power_watts_sum IS NOT NULL",
                params![CLUSTER_NODE_COUNT],
            )?;
            let updated_1d = conn.execute(
                "UPDATE snapshots_1d SET power_watts_sum = power_watts_sum * ?1 \
                 WHERE power_watts_sum IS NOT NULL",
                params![CLUSTER_NODE_COUNT],
            )?;
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('power_backfill_done', ?1)",
                params![CLUSTER_NODE_COUNT as i64],
            )?;
            info!(
                "History backfill: multiplied power by {} for {} 1s + {} 1h + {} 1d rows",
                CLUSTER_NODE_COUNT, updated_1s, updated_1h, updated_1d
            );
        }

        // Legacy rows (pre-seconds/power columns) have NULL sample_count /
        // power_watts_sum; SUM() skips NULLs, so lifetime aggregates
        // under-counted. Backfill: seconds := bucket cadence, power := 0.
        conn.execute(
            "UPDATE snapshots_1d SET sample_count = 86400 WHERE sample_count IS NULL",
            [],
        )?;
        conn.execute(
            "UPDATE snapshots_1d SET power_watts_sum = 0 WHERE power_watts_sum IS NULL",
            [],
        )?;
        conn.execute(
            "UPDATE snapshots_1h SET sample_count = 3600 WHERE sample_count IS NULL",
            [],
        )?;
        conn.execute(
            "UPDATE snapshots_1h SET power_watts_sum = 0 WHERE power_watts_sum IS NULL",
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
        preemptions_total: Option<i64>,
        queue_time_ms: Option<f64>,
        tpot_ms: Option<f64>,
        spec_decode_acceptance_rate: Option<f64>,
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
              kv_cache_pct, prefix_cache_hit, cpu_util, mem_used_pct, preemptions_total,
              queue_time_ms, tpot_ms, spec_decode_acceptance_rate)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
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
                preemptions_total,
                queue_time_ms,
                tpot_ms,
                spec_decode_acceptance_rate,
            ],
        )?;
        Ok(())
    }

    /// Roll up completed hours into hourly records and prune the 1s source data.
    /// Should be called periodically (e.g. once per hour by a background task).
    pub async fn rollup_1s_to_1h(&self) -> rusqlite::Result<u64> {
        let db = self.inner.lock().await;
        // Checkpoint WAL to prevent "disk I/O error" on large rollups.
        let _ = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        // Find all complete hours that haven't been rolled up yet.
        // We compute the latest hour boundary from the data.
        // A "complete hour" is one whose all-60-minutes-worth of data has
        // passed — i.e., the current time has moved past that hour.
        let now_ms = chrono_now_ms();
        let current_hour_start = (now_ms / 3_600_000) * 3_600_000;

        // Roll up every complete hour that has data. The latency columns
        // (ttft_ms_p95, itl_ms_p95, e2e_ms_p95) use AVG() as an approximation
        // of the p95 since SQLite has no built-in PERCENTILE. The column
        // names are kept for API/metrics-contract compatibility. ON CONFLICT
        // DO UPDATE refreshes ALL columns so existing rows are fully
        // backfilled when new 1s data arrives in a subsequent rollup run.
        let rows = db.execute(
            "INSERT INTO snapshots_1h
             (engine_key, bucket_ts,
              total_prompt_tokens, total_gen_tokens, total_requests,
              prompt_tps_avg, prompt_tps_max,
              decode_tps_avg, decode_tps_max,
              ttft_ms_p95, itl_ms_p95, e2e_ms_p95,
              power_watts_sum,
              gpu_util_avg, gpu_temp_avg, gpu_temp_max,
              active_requests_max, queued_requests_max,
              kv_cache_pct_avg, kv_cache_pct_max, prefix_cache_hit_avg,
              cpu_util_avg, sample_count, preemptions_total,
              queue_time_ms_avg, tpot_ms_avg, spec_decode_acceptance_rate_avg)
             SELECT
               engine_key, (ts / 3600000) * 3600000,
               SUM(COALESCE(total_prompt_tokens,0)), SUM(COALESCE(total_gen_tokens,0)),
               SUM(COALESCE(total_requests,0)),
               AVG(prompt_tps), MAX(prompt_tps),
               AVG(decode_tps), MAX(decode_tps),
               AVG(ttft_ms), AVG(itl_ms), AVG(e2e_ms),
               SUM(power_watts),
               AVG(gpu_util), AVG(gpu_temp), MAX(gpu_temp),
               MAX(active_requests), MAX(queued_requests),
               AVG(kv_cache_pct), MAX(kv_cache_pct), AVG(prefix_cache_hit),
               AVG(cpu_util), COUNT(*), MAX(preemptions_total),
               AVG(queue_time_ms), AVG(tpot_ms), AVG(spec_decode_acceptance_rate)
             FROM snapshots_1s
             WHERE ts < ?1
             GROUP BY engine_key, (ts / 3600000)
             ON CONFLICT(engine_key, bucket_ts) DO UPDATE SET
               total_prompt_tokens = excluded.total_prompt_tokens,
               total_gen_tokens = excluded.total_gen_tokens,
               total_requests = excluded.total_requests,
               prompt_tps_avg = excluded.prompt_tps_avg,
               prompt_tps_max = excluded.prompt_tps_max,
               decode_tps_avg = excluded.decode_tps_avg,
               decode_tps_max = excluded.decode_tps_max,
               ttft_ms_p95 = excluded.ttft_ms_p95,
               itl_ms_p95 = excluded.itl_ms_p95,
               e2e_ms_p95 = excluded.e2e_ms_p95,
               power_watts_sum = excluded.power_watts_sum,
               gpu_util_avg = excluded.gpu_util_avg,
               gpu_temp_avg = excluded.gpu_temp_avg,
               gpu_temp_max = excluded.gpu_temp_max,
               active_requests_max = excluded.active_requests_max,
               queued_requests_max = excluded.queued_requests_max,
               kv_cache_pct_avg = excluded.kv_cache_pct_avg,
               kv_cache_pct_max = excluded.kv_cache_pct_max,
               prefix_cache_hit_avg = excluded.prefix_cache_hit_avg,
               cpu_util_avg = excluded.cpu_util_avg,
               sample_count = excluded.sample_count,
               preemptions_total = excluded.preemptions_total,
               queue_time_ms_avg = excluded.queue_time_ms_avg,
               tpot_ms_avg = excluded.tpot_ms_avg",
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
    pub async fn rollup_1h_to_1d(&self, day_offset_ms: Option<i64>) -> rusqlite::Result<u64> {
        let db = self.inner.lock().await;
        let day_offset_ms = match day_offset_ms {
            Some(o) => o,
            None => {
                get_setting_sync(&db, "utc_offset")
                    .ok()
                    .flatten()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0)
                    * 3_600_000
            }
        };
        // Checkpoint WAL to prevent "disk I/O error" on large rollups.
        let _ = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        let now_ms = chrono_now_ms();
        let current_day_start = (now_ms / 86_400_000) * 86_400_000;
        // Key of the current (incomplete) day bucket on the shifted-day grid
        // the daily table uses ((t - offset) floored to UTC day, + offset).
        // The lookback bound is aligned to this grid — NOT to UTC midnights —
        // so a day bucket is either fully re-aggregated or not touched at all.
        // A UTC-midnight bound would straddle day buckets whenever
        // utc_offset != 0 and partially rewrite them on their way out of the
        // window (the same frozen-partial-row defect as pruned sources).
        let shifted_today_start =
            ((now_ms - day_offset_ms) / 86_400_000) * 86_400_000 + day_offset_ms;
        let lookback_start = shifted_today_start - (DAILY_ROLLUP_LOOKBACK_DAYS - 1) * 86_400_000;

        // Merge duplicate daily buckets before the re-aggregation. Rows
        // written before the utc_offset feature was introduced are keyed at
        // UTC midnights, rows written after are keyed on the shifted grid, so
        // the same calendar day can exist twice (double-counted in range
        // sums). Partition rows by the day-bucket formula the rollup itself
        // uses; keep the row with the most samples (tie-break: more prompt
        // tokens) and delete the rest. Idempotent: once a single row remains
        // per day bucket, every row ranks rn = 1 and nothing is deleted.
        // Safe to run on every rollup tick.
        let deduped = db.execute(
            "WITH ranked AS (
                 SELECT rowid AS rid, ROW_NUMBER() OVER (
                     PARTITION BY engine_key,
                                  ((bucket_ts - ?1) / 86400000) * 86400000 + ?1
                     ORDER BY sample_count DESC,
                              COALESCE(total_prompt_tokens, 0) DESC,
                              rowid ASC
                 ) AS rn
                 FROM snapshots_1d
             )
             DELETE FROM snapshots_1d WHERE rowid IN (SELECT rid FROM ranked WHERE rn > 1)",
            params![day_offset_ms],
        )?;
        if deduped > 0 {
            info!("History: merged {} duplicate daily bucket rows", deduped);
        }

        // Same latency-column + ON CONFLICT DO UPDATE pattern as the 1s→1h
        // rollup: ON CONFLICT refreshes ALL columns. The 1h table stores
        // ttft_ms_p95/itl_ms_p95/e2e_ms_p95 (AVG approximation) and
        // queue_time_ms_avg/tpot_ms_avg; we average them across hours for
        // the daily bucket.
        let rows = db.execute(
            "INSERT INTO snapshots_1d
             (engine_key, bucket_ts,
              total_prompt_tokens, total_gen_tokens, total_requests,
              prompt_tps_avg, prompt_tps_max,
              decode_tps_avg, decode_tps_max,
              ttft_ms_p95, itl_ms_p95, e2e_ms_p95,
              power_watts_sum,
              gpu_util_avg, gpu_temp_avg, gpu_temp_max,
              active_requests_max, queued_requests_max,
              kv_cache_pct_avg, kv_cache_pct_max, prefix_cache_hit_avg,
              cpu_util_avg, sample_count, preemptions_total,
              queue_time_ms_avg, tpot_ms_avg, spec_decode_acceptance_rate_avg)
             SELECT
              engine_key, ((bucket_ts - ?2) / 86400000) * 86400000 + ?2,
               SUM(COALESCE(total_prompt_tokens,0)), SUM(COALESCE(total_gen_tokens,0)),
               SUM(COALESCE(total_requests,0)),
               AVG(prompt_tps_avg), MAX(prompt_tps_max),
               AVG(decode_tps_avg), MAX(decode_tps_max),
               AVG(ttft_ms_p95), AVG(itl_ms_p95), AVG(e2e_ms_p95),
               SUM(power_watts_sum),
               AVG(gpu_util_avg), AVG(gpu_temp_avg), MAX(gpu_temp_max),
               MAX(active_requests_max), MAX(queued_requests_max),
               AVG(kv_cache_pct_avg), MAX(kv_cache_pct_max), AVG(prefix_cache_hit_avg),
               AVG(cpu_util_avg), SUM(sample_count), MAX(preemptions_total),
               AVG(queue_time_ms_avg), AVG(tpot_ms_avg), AVG(spec_decode_acceptance_rate_avg)
               FROM snapshots_1h
              WHERE bucket_ts < ?1 AND bucket_ts >= ?3
              GROUP BY engine_key, ((bucket_ts - ?2) / 86400000) * 86400000 + ?2
             ON CONFLICT(engine_key, bucket_ts) DO UPDATE SET
               total_prompt_tokens = excluded.total_prompt_tokens,
               total_gen_tokens = excluded.total_gen_tokens,
               total_requests = excluded.total_requests,
               prompt_tps_avg = excluded.prompt_tps_avg,
               prompt_tps_max = excluded.prompt_tps_max,
               decode_tps_avg = excluded.decode_tps_avg,
               decode_tps_max = excluded.decode_tps_max,
               ttft_ms_p95 = excluded.ttft_ms_p95,
               itl_ms_p95 = excluded.itl_ms_p95,
               e2e_ms_p95 = excluded.e2e_ms_p95,
               power_watts_sum = excluded.power_watts_sum,
               gpu_util_avg = excluded.gpu_util_avg,
               gpu_temp_avg = excluded.gpu_temp_avg,
               gpu_temp_max = excluded.gpu_temp_max,
               active_requests_max = excluded.active_requests_max,
               queued_requests_max = excluded.queued_requests_max,
               kv_cache_pct_avg = excluded.kv_cache_pct_avg,
               kv_cache_pct_max = excluded.kv_cache_pct_max,
               prefix_cache_hit_avg = excluded.prefix_cache_hit_avg,
               cpu_util_avg = excluded.cpu_util_avg,
               sample_count = excluded.sample_count,
               preemptions_total = excluded.preemptions_total,
               queue_time_ms_avg = excluded.queue_time_ms_avg,
               tpot_ms_avg = excluded.tpot_ms_avg,
               spec_decode_acceptance_rate_avg = excluded.spec_decode_acceptance_rate_avg",
            params![current_day_start, day_offset_ms, lookback_start],
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
    ///
    /// When the range spans one or more completed days **plus** the current
    /// (incomplete) day, we query the daily table for completed days and the
    /// hourly table for today, then merge.  This ensures data from the current
    /// day — which has not yet been rolled up into the daily table — is not
    /// lost.  If the daily query returns no rows (range is entirely within
    /// today) we fall through to the existing hourly → raw chain for the full
    /// range.
    pub async fn query_summary(
        &self,
        engine_key: &str,
        since_ms: i64,
        until_ms: i64,
    ) -> rusqlite::Result<Option<HistorySummary>> {
        let db = self.inner.lock().await;

        let now_ms = chrono_now_ms();
        // LOCAL (user) midnight, not UTC: daily rows are ET-keyed.
        let utc_offset_h: i64 = get_setting_sync(&db, "utc_offset")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let day_offset_ms = utc_offset_h * 3_600_000;
        let shifted = now_ms - day_offset_ms;
        let start_of_today = (shifted / 86_400_000) * 86_400_000 + day_offset_ms;

        // ── Path 1: daily (completed days) + hourly (today) merge ──────
        // Only attempt the merge when the range actually starts before today.
        if since_ms < start_of_today {
            let daily = query_summary_one_table(
                &db,
                engine_key,
                since_ms,
                // completed days only: up to but not including today's start
                start_of_today - 1,
                ("snapshots_1d", "daily", "bucket_ts"),
            )?;

            if let Some(daily) = daily {
                // Query hourly for today's portion of the range.
                let hourly = query_summary_one_table(
                    &db,
                    engine_key,
                    start_of_today,
                    until_ms,
                    ("snapshots_1h", "hourly", "bucket_ts"),
                )?;
                if let Some(hourly) = hourly {
                    return Ok(Some(merge_summaries(&daily, &hourly)));
                }
                // No hourly data for today — return daily alone.
                return Ok(Some(daily));
            }
            // Daily returned nothing → fall through to the single-table chain.
        }

        // ── Path 2: single-table fallback (hourly → raw) ───────────────
        let try_tables = [
            ("snapshots_1h", "hourly", "bucket_ts"),
            ("snapshots_1s", "raw", "ts"),
        ];

        for table_info in &try_tables {
            let result = query_summary_one_table(&db, engine_key, since_ms, until_ms, *table_info)?;
            if let Some(summary) = result {
                return Ok(Some(summary));
            }
        }

        Ok(None)
    }

    /// Query time-series data points for a given metric and time window.
    ///
    /// The table is chosen by the time range:
    ///   - ≤ 1 hour: `snapshots_1s` (raw, 1s resolution)
    ///   - ≤ 24 hours: `snapshots_1h` (hourly buckets)
    ///   - > 24 hours: `snapshots_1d` (daily buckets)
    ///
    /// Returns one data point per row, ordered by timestamp ascending.
    /// An empty vec means there is no data for the given engine/metric in
    /// the requested window (or the metric has no aggregate column for the
    /// chosen table, e.g. `mem_used_pct` over a > 1 h window).
    pub async fn query_timeseries(
        &self,
        engine_key: &str,
        metric: &str,
        since_ms: i64,
        until_ms: i64,
    ) -> rusqlite::Result<Vec<TimeSeriesPoint>> {
        let db = self.inner.lock().await;

        const HOUR_MS: i64 = 3_600_000;
        const DAY_MS: i64 = 86_400_000;

        // Resolve the metric to column names for the raw (1s) and aggregated
        // (1h/1d) tables. `None` for the aggregate column means the metric is
        // only available at 1s resolution (e.g. mem_used_pct).
        let (raw_col, agg_col): (&str, Option<&str>) = match metric {
            "prompt_tps" => ("prompt_tps", Some("prompt_tps_avg")),
            "decode_tps" => ("decode_tps", Some("decode_tps_avg")),
            "ttft_ms" => ("ttft_ms", Some("ttft_ms_p95")),
            "itl_ms" => ("itl_ms", Some("itl_ms_p95")),
            "e2e_ms" => ("e2e_ms", Some("e2e_ms_p95")),
            "active_requests" => ("active_requests", Some("active_requests_max")),
            "queued_requests" => ("queued_requests", Some("queued_requests_max")),
            "total_requests" => ("total_requests", Some("total_requests")),
            "kv_cache_pct" => ("kv_cache_pct", Some("kv_cache_pct_max")),
            "prefix_cache_hit" => ("prefix_cache_hit", Some("prefix_cache_hit_avg")),
            "gpu_util" => ("gpu_util", Some("gpu_util_avg")),
            "gpu_temp" => ("gpu_temp", Some("gpu_temp_max")),
            "power_watts" => ("power_watts", Some("power_watts_sum")),
            "cpu_util" => ("cpu_util", Some("cpu_util_avg")),
            "mem_used_pct" => ("mem_used_pct", None),
            "preemptions_total" => ("preemptions_total", Some("preemptions_total")),
            "queue_time_ms" => ("queue_time_ms", Some("queue_time_ms_avg")),
            "tpot_ms" => ("tpot_ms", Some("tpot_ms_avg")),
            "spec_decode_acceptance_rate" => (
                "spec_decode_acceptance_rate",
                Some("spec_decode_acceptance_rate_avg"),
            ),
            _ => return Ok(Vec::new()),
        };

        // Gauge metrics are instantaneous snapshots where averaging hides
        // peaks; bucket them with MAX to match the rollup tables. All other
        // metrics (rates, latencies, utilization) use AVG.
        let agg_func = agg_func_for_metric(metric);

        let range_ms = until_ms.saturating_sub(since_ms);

        if range_ms <= HOUR_MS {
            // For ranges ≤ 1h we query BOTH the 1s table (raw data for the
            // current, not-yet-rolled-up stretch) AND the 1h table (completed
            // hour buckets whose 1s data has already been rolled up and
            // pruned).  This prevents empty charts when the rollup has
            // already deleted the 1s data for the earlier part of the
            // requested range — but a completed-hour bucket is only used
            // when the raw 1s data does NOT already reach back to the
            // window's start (see the coverage check below).
            let bucket_ms = bucket_size_ms(range_ms);
            let sql_raw = format!(
                "SELECT MAX((ts / {bucket_ms}) * {bucket_ms}, ?2) AS bucket_ts, \
                 MIN(ts) AS min_ts, \
                 {agg_func}({col}) AS value \
                 FROM snapshots_1s \
                 WHERE engine_key = ?1 AND ts >= ?2 AND ts <= ?3 \
                 GROUP BY bucket_ts \
                 ORDER BY bucket_ts ASC",
                col = raw_col,
                bucket_ms = bucket_ms,
                agg_func = agg_func,
            );
            let mut stmt = db.prepare(&sql_raw)?;
            // (bucket_ts, min sample ts in bucket, aggregated value)
            let raw_points = stmt
                .query_map(params![engine_key, since_ms, until_ms], |r| {
                    let bucket_ts: i64 = r.get(0)?;
                    let min_ts: i64 = r.get(1)?;
                    let val: Option<f64> = r.get(2)?;
                    Ok((bucket_ts, min_ts, val.unwrap_or(0.0)))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            // Raw points go in first: when both sources produce the same
            // timestamp after the sort below, the raw 1s value wins.
            let mut points: Vec<TimeSeriesPoint> = raw_points
                .iter()
                .map(|(bucket_ts, _, value)| TimeSeriesPoint {
                    timestamp_ms: *bucket_ts,
                    value: *value,
                })
                .collect();

            // Completed-hour rollup buckets within the range.
            if let Some(agg) = agg_col {
                let value_expr = if metric == "power_watts" {
                    format!("{agg} / NULLIF(sample_count, 0)")
                } else {
                    agg.to_owned()
                };
                let sql_agg = format!(
                    "SELECT bucket_ts, {value_expr} FROM snapshots_1h \
                     WHERE engine_key = ?1 AND bucket_ts >= ?2 AND bucket_ts <= ?3 \
                     ORDER BY bucket_ts ASC",
                );
                let mut stmt = db.prepare(&sql_agg)?;
                // Include the complete hour STRADDLING `since`: when the
                // range starts mid-hour and the 1s data for that stretch was
                // already rolled up + pruned, that earlier bucket is the
                // only data covering [since, next-hour-start).
                let since_hour = (since_ms / 3_600_000) * 3_600_000;
                let agg_points = stmt
                    .query_map(params![engine_key, since_hour, until_ms], |r| {
                        let bucket_ts: i64 = r.get(0)?;
                        // Clamp the straddling bucket's timestamp INTO the
                        // requested window. Its value legitimately covers
                        // [since, next-hour-start), but reporting it at its
                        // raw (earlier) timestamp makes clients with a fixed
                        // x-domain expand that domain backwards — the 1h
                        // chart rendered a ~2h axis with data scrunched into
                        // the right half.
                        let ts = bucket_ts.max(since_ms);
                        let val: Option<f64> = r.get(1)?;
                        Ok((bucket_ts, ts, val.unwrap_or(0.0)))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                // Skip the full-hour average when raw 1s rows already reach
                // back to the window's start: with ~1h of 1s retention a
                // 60s/300s window almost always has raw rows, and mixing in
                // the bucket's hour-wide average puts an artificial
                // value-jump point at the window's left edge. "Reached the
                // start" means the earliest raw sample sits within one
                // bucket width of `since_ms`. The raw query filters
                // `ts >= since_ms`, so min_ts >= since_ms always — a
                // `<= since_ms` comparison would degenerate to
                // exact-millisecond equality, which continuous 1s sampling
                // (arbitrary sub-second phase vs the client's Date.now())
                // essentially never produces. `min_ts < since_ms +
                // bucket_ms` holds for continuous coverage (the first
                // sample lands within ~1s of `since` and bucket_ms >= 1s)
                // and fails whenever raw starts late (post-prune or engine
                // restart), in which case the hour-wide average still
                // fills the uncovered stretch.
                let covered = raw_points
                    .iter()
                    .any(|(_, min_ts, _)| *min_ts < since_ms + bucket_ms);
                for (_bucket_ts, ts, value) in agg_points {
                    if !covered {
                        points.push(TimeSeriesPoint {
                            timestamp_ms: ts,
                            value,
                        });
                    }
                }
            }

            // Sort ascending; drop duplicate timestamps (the raw point,
            // inserted first, wins the stable sort and the dedupe).
            points.sort_by_key(|p| p.timestamp_ms);
            points.dedup_by(|a, b| a.timestamp_ms == b.timestamp_ms);
            Ok(points)
        } else {
            // Aggregated table (1h or 1d).
            let agg = match agg_col {
                Some(c) => c,
                None => return Ok(Vec::new()),
            };
            let (table, ts_col) = if range_ms <= DAY_MS {
                ("snapshots_1h", "bucket_ts")
            } else {
                ("snapshots_1d", "bucket_ts")
            };

            // For power_watts the aggregated column is a sum; divide by
            // sample_count to recover the average power for the bucket.
            let value_expr = if metric == "power_watts" {
                format!("{agg} / NULLIF(sample_count, 0)")
            } else {
                agg.to_owned()
            };

            // Part 1: query the rollup table for completed periods.
            let sql = format!(
                "SELECT {ts_col}, {value_expr} FROM {table} \
                 WHERE engine_key = ?1 AND {ts_col} >= ?2 AND {ts_col} <= ?3 \
                 ORDER BY {ts_col} ASC",
            );
            let mut stmt = db.prepare(&sql)?;
            let mut points: Vec<TimeSeriesPoint> = stmt
                .query_map(params![engine_key, since_ms, until_ms], |r| {
                    let ts: i64 = r.get(0)?;
                    let val: Option<f64> = r.get(1)?;
                    Ok(TimeSeriesPoint {
                        timestamp_ms: ts,
                        value: val.unwrap_or(0.0),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            // Part 2: for 1h table, also query raw 1s data for the current
            // incomplete hour (data that hasn't been rolled up yet). This
            // ensures the chart shows recent activity even when the rollup
            // hasn't run or has failed.
            if table == "snapshots_1h" {
                let now_ms = chrono_now_ms();
                let current_hour_start = (now_ms / 3_600_000) * 3_600_000;
                if until_ms >= current_hour_start {
                    // Remove any stale 1h points for the current hour (from
                    // partial rollups).
                    points.retain(|p| p.timestamp_ms < current_hour_start);

                    // Query 1s data for the current hour, bucketed to 1h.
                    let sql_raw = format!(
                        "SELECT (ts / 3600000) * 3600000 AS bucket_ts, \
                         {agg_func}({col}) AS value \
                         FROM snapshots_1s \
                         WHERE engine_key = ?1 AND ts >= ?2 AND ts <= ?3 \
                         GROUP BY bucket_ts \
                         ORDER BY bucket_ts ASC",
                        col = raw_col,
                        agg_func = agg_func,
                    );
                    let mut stmt2 = db.prepare(&sql_raw)?;
                    let raw_points: Vec<TimeSeriesPoint> = stmt2
                        .query_map(params![engine_key, current_hour_start, until_ms], |r| {
                            let ts: i64 = r.get(0)?;
                            let val: Option<f64> = r.get(1)?;
                            Ok(TimeSeriesPoint {
                                timestamp_ms: ts,
                                value: val.unwrap_or(0.0),
                            })
                        })?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    points.extend(raw_points);
                    points.sort_by_key(|p| p.timestamp_ms);
                }
            }

            Ok(points)
        }
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

/// Run a single summary query against one rollup/raw table.
///
/// Returns `Ok(None)` when the table has no matching rows or when the rows
/// contain no meaningful data (all-zero deltas).  Returns `Ok(Some(summary))`
/// when usable data is found.
///
/// # Peak KV cache correctness
///
/// The peak KV cache value is computed as `MAX(kv_cache_pct_max)` for rollup
/// tables or `MAX(kv_cache_pct)` for the raw 1s table.  This is correct because
/// the rollup chain preserves maxima at each level:
///   1. `insert_1s()` stores the instantaneous `kv_cache_pct` in `snapshots_1s`.
///   2. `rollup_1s_to_1h()` computes `MAX(kv_cache_pct)` per hour bucket,
///      storing it as `kv_cache_pct_max` in `snapshots_1h`.
///   3. `rollup_1h_to_1d()` computes `MAX(kv_cache_pct_max)` per day bucket,
///      storing it as `kv_cache_pct_max` in `snapshots_1d`.
///   4. `query_summary_one_table` takes `MAX(kv_cache_pct_max)` across all
///      buckets in the requested time range.
///   5. When the range spans completed days + today, `merge_summaries` uses
///      `merge_opt_max` (MAX of daily peak and hourly peak) to combine them.
///
/// Thus the single highest KV cache value recorded in the time range is always
/// returned, regardless of which table(s) serve the query.
/// Fetch a settings value synchronously from the open connection.
fn get_setting_sync(db: &rusqlite::Connection, key: &str) -> rusqlite::Result<Option<String>> {
    db.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

fn query_summary_one_table(
    db: &Connection,
    engine_key: &str,
    since_ms: i64,
    until_ms: i64,
    (table, source, ts_col): (&str, &'static str, &str),
) -> rusqlite::Result<Option<HistorySummary>> {
    let gauge_suffix = if source == "raw" { "" } else { "_max" };
    let power_suffix = if source == "raw" { "" } else { "_sum" };
    let sql = if source == "raw" {
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
               COUNT(*),
               MAX(kv_cache_pct),
               AVG(kv_cache_pct),
               COALESCE(MAX(preemptions_total),0) - COALESCE(MIN(preemptions_total),0)
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
                MAX(active_requests{g1}),
                MAX(queued_requests{g2}),
                COALESCE(SUM(power_watts{pw}),0),
                COALESCE(SUM(sample_count),
                    CAST((MAX({tsc}) - MIN({tsc})) / 3600000 + 1 AS INTEGER)),
                MAX(kv_cache_pct_max),
                SUM(kv_cache_pct_avg * sample_count) / NULLIF(SUM(sample_count), 0),
                COALESCE(MAX(preemptions_total),0) - COALESCE(MIN(preemptions_total),0)
              FROM {table}
              WHERE engine_key = ?1 AND {ts1} >= ?2 AND {ts2} <= ?3",
            g1 = gauge_suffix,
            g2 = gauge_suffix,
            pw = power_suffix,
            tsc = ts_col,
            table = table,
            ts1 = ts_col,
            ts2 = ts_col,
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
        let peak_kv_cache_pct: Option<f64> = r.get::<_, Option<f64>>(9)?;
        let avg_kv_cache_pct: Option<f64> = r.get::<_, Option<f64>>(10)?;
        let total_preemptions: i64 = r.get::<_, Option<i64>>(11)?.unwrap_or(0);
        Ok(HistorySummary {
            delta_prompt_tokens: delta_prompt,
            delta_gen_tokens: delta_gen,
            total_requests: total_reqs,
            avg_decode_tps: avg_decode,
            avg_prompt_tps: avg_prompt,
            peak_active_requests: peak_active,
            peak_queued_requests: peak_queued,
            peak_kv_cache_pct,
            avg_kv_cache_pct,
            total_preemptions: Some(total_preemptions),
            power_kwh: power_sum / 3600.0 / 1000.0,
            total_seconds: Some(count as f64),
            source_table: source,
        })
    });

    match result {
        Ok(summary) => {
            if summary.delta_prompt_tokens > 0
                || summary.delta_gen_tokens > 0
                || summary.total_requests > 0
            {
                Ok(Some(summary))
            } else {
                Ok(None)
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => {
            tracing::warn!("History query failed on {}: {}", table, e);
            Ok(None)
        }
    }
}

/// Merge two [`HistorySummary`] values (e.g. daily + hourly) into one.
///
/// - Peaks (kv_cache, active, queued) → MAX of the two
/// - Sums (tokens, requests, power, seconds, preemptions) → SUM of the two
/// - Averages (decode_tps, prompt_tps, kv_cache_pct) → weighted by sample_count
fn merge_summaries(daily: &HistorySummary, hourly: &HistorySummary) -> HistorySummary {
    let daily_samples = daily.total_seconds.unwrap_or(0.0);
    let hourly_samples = hourly.total_seconds.unwrap_or(0.0);
    let total_samples = daily_samples + hourly_samples;

    let weighted = |d: f64, h: f64| -> f64 {
        if total_samples > 0.0 {
            (d * daily_samples + h * hourly_samples) / total_samples
        } else {
            0.0
        }
    };

    let merge_opt_max = |a: Option<f64>, b: Option<f64>| -> Option<f64> {
        match (a, b) {
            (Some(x), Some(y)) => Some(x.max(y)),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    };

    let merge_opt_weighted = |a: Option<f64>, b: Option<f64>| -> Option<f64> {
        match (a, b) {
            (Some(x), Some(y)) => {
                if total_samples > 0.0 {
                    Some((x * daily_samples + y * hourly_samples) / total_samples)
                } else {
                    Some((x + y) / 2.0)
                }
            }
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    };

    let daily_preemptions = daily.total_preemptions.unwrap_or(0);
    let hourly_preemptions = hourly.total_preemptions.unwrap_or(0);

    HistorySummary {
        delta_prompt_tokens: daily.delta_prompt_tokens + hourly.delta_prompt_tokens,
        delta_gen_tokens: daily.delta_gen_tokens + hourly.delta_gen_tokens,
        total_requests: daily.total_requests + hourly.total_requests,
        avg_decode_tps: weighted(daily.avg_decode_tps, hourly.avg_decode_tps),
        avg_prompt_tps: weighted(daily.avg_prompt_tps, hourly.avg_prompt_tps),
        peak_active_requests: daily.peak_active_requests.max(hourly.peak_active_requests),
        peak_queued_requests: daily.peak_queued_requests.max(hourly.peak_queued_requests),
        peak_kv_cache_pct: merge_opt_max(daily.peak_kv_cache_pct, hourly.peak_kv_cache_pct),
        avg_kv_cache_pct: merge_opt_weighted(daily.avg_kv_cache_pct, hourly.avg_kv_cache_pct),
        total_preemptions: Some(daily_preemptions + hourly_preemptions),
        power_kwh: daily.power_kwh + hourly.power_kwh,
        total_seconds: Some(total_samples),
        source_table: "mixed",
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
    /// Peak KV cache utilization (%) over the window.
    pub peak_kv_cache_pct: Option<f64>,
    /// Average KV cache utilization (%) over the window.
    pub avg_kv_cache_pct: Option<f64>,
    /// Preemptions that occurred during the window (MAX − MIN of the lifetime counter).
    pub total_preemptions: Option<i64>,
    /// Total energy consumption in kilowatt-hours.
    pub power_kwh: f64,
    /// Total seconds represented by the data (for calculating hours alive).
    pub total_seconds: Option<f64>,
    /// Which internal table satisfied the query (raw | hourly | daily).
    pub source_table: &'static str,
}

/// A single time-series data point returned by [`HistoryDb::query_timeseries`].
#[derive(serde::Serialize, Clone, Debug)]
pub struct TimeSeriesPoint {
    pub timestamp_ms: i64,
    pub value: f64,
}

/// Map a metric name to the SQL aggregation function used when bucketing raw
/// 1s data. Gauge metrics (instantaneous snapshots) use MAX so peaks aren't
/// hidden by averaging, matching the rollup tables. Continuous and rate
/// metrics use AVG, preserving the previous downsampling behavior.
fn agg_func_for_metric(metric: &str) -> &'static str {
    match metric {
        "active_requests"
        | "queued_requests"
        | "kv_cache_pct"
        | "prefix_cache_hit"
        | "gpu_util"
        | "gpu_temp"
        | "spec_decode_acceptance_rate" => "MAX",
        _ => "AVG",
    }
}

/// Compute the bucket size (ms) for aggregating raw 1s data in
/// [`HistoryDb::query_timeseries`]. Targets ~360 buckets across the range,
/// with a 1-second floor so short ranges keep 1s resolution.
fn bucket_size_ms(range_ms: i64) -> i64 {
    (range_ms / 360).max(1000)
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

    /// (ttft_ms_p95, itl_ms_p95, e2e_ms_p95, queue_time_ms_avg, tpot_ms_avg)
    type LatencyRow = (
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    );

    /// Read the five latency columns from a rollup table for one engine row.
    fn read_latency(
        conn: &Connection,
        table: &str,
        engine_key: &str,
        bucket_ts: Option<i64>,
    ) -> LatencyRow {
        let sql = match bucket_ts {
            Some(_) => format!(
                "SELECT ttft_ms_p95, itl_ms_p95, e2e_ms_p95, queue_time_ms_avg, tpot_ms_avg \
                 FROM {table} WHERE engine_key = ?1 AND bucket_ts = ?2"
            ),
            None => format!(
                "SELECT ttft_ms_p95, itl_ms_p95, e2e_ms_p95, queue_time_ms_avg, tpot_ms_avg \
                 FROM {table} WHERE engine_key = ?1"
            ),
        };
        let mapper = |r: &rusqlite::Row| -> rusqlite::Result<LatencyRow> {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        };
        match bucket_ts {
            Some(b) => conn
                .query_row(&sql, params![engine_key, b], mapper)
                .unwrap(),
            None => conn.query_row(&sql, params![engine_key], mapper).unwrap(),
        }
    }

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
                mem_used_pct       REAL,
                preemptions_total  INTEGER,
                queue_time_ms      REAL,
                tpot_ms            REAL,
                spec_decode_acceptance_rate REAL
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
                kv_cache_pct_max    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                preemptions_total   INTEGER,
                queue_time_ms_avg   REAL,
                tpot_ms_avg         REAL,
                spec_decode_acceptance_rate_avg REAL,
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
                kv_cache_pct_max    REAL,
                prefix_cache_hit_avg REAL,
                cpu_util_avg        REAL,
                sample_count        INTEGER NOT NULL,
                preemptions_total   INTEGER,
                queue_time_ms_avg   REAL,
                tpot_ms_avg         REAL,
                spec_decode_acceptance_rate_avg REAL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_1d_unique ON snapshots_1d(engine_key, bucket_ts);
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
            now - 1000,
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
            Some(0),
            None,
            None,
            None,
        )
        .await
        .unwrap();
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
            Some(3),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let summary = db.query_summary(key, now - 2000, now + 1000).await.unwrap();
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(s.delta_prompt_tokens, 200);
        assert_eq!(s.delta_gen_tokens, 400);
        assert_eq!(s.total_requests, 10);
        assert_eq!(s.source_table, "raw");
        assert!(s.power_kwh > 0.0);
        assert_eq!(
            s.total_preemptions,
            Some(3),
            "delta: MAX(0,3) - MIN(0,3) = 3"
        );
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
            Some(0),
            None,
            None,
            None,
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
            Some(2),
            None,
            None,
            None,
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
            Some(5),
            None,
            None,
            None,
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
        assert_eq!(
            s.total_preemptions,
            Some(3),
            "delta of preemptions: MAX(2,5) - MIN(2,5) = 3"
        );
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

    /// The one-time power backfill migration should multiply all existing
    /// power values by the cluster size (4) when it runs, and be idempotent
    /// on subsequent runs (guarded by the `power_backfill_done` settings key).
    #[tokio::test]
    async fn test_power_backfill_multiplies_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .unwrap();
        // First migrate creates the schema and runs the backfill (which is a
        // no-op on an empty DB but still marks `power_backfill_done`).
        HistoryDb::migrate(&conn).unwrap();

        // Simulate pre-existing data recorded before the backfill: clear the
        // marker, insert rows with known power values, then re-run migrate.
        conn.execute("DELETE FROM settings WHERE key = 'power_backfill_done'", [])
            .unwrap();
        // Insert a 1s row with a known power value.
        conn.execute(
            "INSERT INTO snapshots_1s (engine_key, ts, power_watts) VALUES ('e1', 1000, 100.0)",
            [],
        )
        .unwrap();
        // Insert a 1h row with a known power_watts_sum.
        conn.execute(
            "INSERT INTO snapshots_1h (engine_key, bucket_ts, power_watts_sum, sample_count) \
             VALUES ('e1', 3600000, 200.0, 60)",
            [],
        )
        .unwrap();
        // Insert a 1d row with a known power_watts_sum.
        conn.execute(
            "INSERT INTO snapshots_1d (engine_key, bucket_ts, power_watts_sum, sample_count) \
             VALUES ('e1', 86400000, 300.0, 1440)",
            [],
        )
        .unwrap();

        // Run migrate again — this time the backfill should fire on the data.
        HistoryDb::migrate(&conn).unwrap();

        // All power values should be multiplied by 4.
        let p1s: f64 = conn
            .query_row(
                "SELECT power_watts FROM snapshots_1s WHERE engine_key='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p1s, 400.0, "1s power should be 100 * 4");
        let p1h: f64 = conn
            .query_row(
                "SELECT power_watts_sum FROM snapshots_1h WHERE engine_key='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p1h, 800.0, "1h power should be 200 * 4");
        let p1d: f64 = conn
            .query_row(
                "SELECT power_watts_sum FROM snapshots_1d WHERE engine_key='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p1d, 1200.0, "1d power should be 300 * 4");

        // The settings key should be set.
        let done: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key='power_backfill_done'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(done, "4");

        // Run migrate a third time — should NOT multiply again.
        HistoryDb::migrate(&conn).unwrap();
        let p1s_again: f64 = conn
            .query_row(
                "SELECT power_watts FROM snapshots_1s WHERE engine_key='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p1s_again, 400.0, "backfill must be idempotent");
    }

    // -----------------------------------------------------------------
    // query_timeseries tests
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn test_timeseries_1s_returns_data_points() {
        let db = test_db();
        let key = "ts-engine";
        // Use a fixed base so the range logic is deterministic.
        let base = 1_700_000_000_000i64;

        db.insert_1s(
            key,
            base,
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
            Some(3),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        db.insert_1s(
            key,
            base + 1000,
            Some(110),
            Some(210),
            Some(6),
            Some(55.0),
            Some(32.0),
            Some(12.0),
            Some(6.0),
            Some(110.0),
            Some(160.0),
            Some(82.0),
            Some(46.0),
            Some(4),
            Some(2),
            Some(0.8),
            Some(0.15),
            Some(62.0),
            Some(52.0),
            Some(5),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Range of 2 seconds → ≤ 1 hour → raw 1s table.
        let points = db
            .query_timeseries(key, "decode_tps", base, base + 2000)
            .await
            .unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].timestamp_ms, base);
        assert!((points[0].value - 30.0).abs() < f64::EPSILON);
        assert_eq!(points[1].timestamp_ms, base + 1000);
        assert!((points[1].value - 32.0).abs() < f64::EPSILON);
    }

    /// Gauge metrics (instantaneous snapshots) must bucket with MAX while
    /// rate/latency metrics use AVG, matching the rollup tables.
    #[test]
    fn test_agg_func_for_metric() {
        for metric in [
            "active_requests",
            "queued_requests",
            "kv_cache_pct",
            "prefix_cache_hit",
            "gpu_util",
            "gpu_temp",
        ] {
            assert_eq!(
                agg_func_for_metric(metric),
                "MAX",
                "{metric} should use MAX"
            );
        }
        for metric in [
            "prompt_tps",
            "decode_tps",
            "ttft_ms",
            "itl_ms",
            "e2e_ms",
            "queue_time_ms",
            "tpot_ms",
            "cpu_util",
            "mem_used_pct",
            "power_watts",
        ] {
            assert_eq!(
                agg_func_for_metric(metric),
                "AVG",
                "{metric} should use AVG"
            );
        }
        assert_eq!(agg_func_for_metric("unknown_metric"), "AVG");
    }

    /// Bucket size targets ~360 buckets per range, never below 1 second.
    #[test]
    fn test_bucket_size_ms() {
        assert_eq!(
            bucket_size_ms(3_600_000),
            10_000,
            "1h -> 360 buckets of 10s"
        );
        assert_eq!(
            bucket_size_ms(900_000),
            2_500,
            "15min -> 360 buckets of 2.5s"
        );
        assert_eq!(
            bucket_size_ms(2_000),
            1_000,
            "short range keeps the 1s floor"
        );
        assert_eq!(bucket_size_ms(0), 1_000, "zero range uses the 1s floor");
    }

    /// 1s data for a ≤1h range is aggregated into buckets: gauge metrics
    /// (active_requests) report the MAX within each bucket, rate metrics
    /// (prompt_tps) the AVG.
    #[tokio::test]
    async fn test_timeseries_1s_bucketing_gauge_max_rate_avg() {
        let db = test_db();
        let key = "bucket-engine";
        // base is a multiple of 10s so the three 1s rows below land in the
        // same 10s bucket for a 1h range (bucket_ms = 3600000/360 = 10000).
        let base = 1_700_000_000_000i64;

        for (i, (active, tps)) in [(3i64, 10.0f64), (4, 20.0), (5, 30.0)].iter().enumerate() {
            db.insert_1s(
                key,
                base + 1000 * i as i64,
                Some(0),
                Some(0),
                Some(0),
                Some(*tps),
                Some(0.0),
                None,
                None,
                None,
                None,
                None,
                None,
                Some(*active),
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
        }

        let points = db
            .query_timeseries(key, "active_requests", base, base + 3_600_000)
            .await
            .unwrap();
        assert_eq!(
            points.len(),
            1,
            "three 1s rows should collapse to one bucket"
        );
        assert_eq!(points[0].timestamp_ms, base);
        assert_eq!(points[0].value, 5.0, "gauge should use MAX(3,4,5)");

        let points = db
            .query_timeseries(key, "prompt_tps", base, base + 3_600_000)
            .await
            .unwrap();
        assert_eq!(
            points.len(),
            1,
            "three 1s rows should collapse to one bucket"
        );
        assert_eq!(points[0].timestamp_ms, base);
        assert!(
            (points[0].value - 20.0).abs() < f64::EPSILON,
            "rate should use AVG(10,20,30)=20, got {}",
            points[0].value
        );
    }

    #[tokio::test]
    async fn test_timeseries_nonexistent_engine_returns_empty() {
        let db = test_db();
        let points = db
            .query_timeseries("ghost", "prompt_tps", 0, 1_000_000)
            .await
            .unwrap();
        assert!(points.is_empty());
    }

    #[tokio::test]
    async fn test_timeseries_unknown_metric_returns_empty() {
        let db = test_db();
        let points = db
            .query_timeseries("any", "bogus_metric", 0, 1_000_000)
            .await
            .unwrap();
        assert!(points.is_empty());
    }

    #[tokio::test]
    async fn test_timeseries_1h_aggregation() {
        let db = test_db();
        // Insert directly into snapshots_1h (bypassing rollup, which is
        // time-of-day dependent and prunes 1s data).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES ('e1', 3600000, 100.0, 50.0, 60)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES ('e1', 7200000, 120.0, 55.0, 60)",
                [],
            )
            .unwrap();
        }

        // Range = 7 200 000 ms = 2 hours → > 1 hour, ≤ 24 hours → 1h table.
        let points = db
            .query_timeseries("e1", "prompt_tps", 0, 7_200_000)
            .await
            .unwrap();
        assert_eq!(points.len(), 2, "should return 2 hourly buckets");
        assert_eq!(points[0].timestamp_ms, 3_600_000);
        assert!((points[0].value - 100.0).abs() < f64::EPSILON);
        assert_eq!(points[1].timestamp_ms, 7_200_000);
        assert!((points[1].value - 120.0).abs() < f64::EPSILON);

        // decode_tps should use decode_tps_avg from the 1h table.
        let points = db
            .query_timeseries("e1", "decode_tps", 0, 7_200_000)
            .await
            .unwrap();
        assert_eq!(points.len(), 2);
        assert!((points[0].value - 50.0).abs() < f64::EPSILON);
        assert!((points[1].value - 55.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_timeseries_power_watts_averaged_in_aggregated() {
        let db = test_db();
        {
            let conn = db.inner.lock().await;
            // power_watts_sum = 6000.0, sample_count = 60 → avg = 100.0
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, power_watts_sum, sample_count) \
                 VALUES ('e1', 3600000, 6000.0, 60)",
                [],
            )
            .unwrap();
        }

        // Range > 1 hour → 1h table.
        let points = db
            .query_timeseries("e1", "power_watts", 0, 7_200_000)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].timestamp_ms, 3_600_000);
        assert!(
            (points[0].value - 100.0).abs() < 0.001,
            "power_watts should be sum/sample_count = 100.0, got {}",
            points[0].value
        );
    }

    /// When querying a >1h range (which selects the 1h table), the current
    /// incomplete hour may not have been rolled up yet — or the rollup may
    /// have failed. The query must fall back to raw 1s data for the current
    /// hour so the chart shows recent activity instead of a gap on the right.
    #[tokio::test]
    async fn test_timeseries_1h_range_includes_current_hour_from_1s() {
        let db = test_db();
        let key = "gap-engine";
        let now = chrono_now_ms();
        let current_hour_start = (now / 3_600_000) * 3_600_000;

        // Insert a completed-hour row in the 1h table (2 hours ago).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 100.0, 50.0, 60)",
                params![key, current_hour_start - 3_600_000],
            )
            .unwrap();
        }

        // Insert raw 1s data for the CURRENT (incomplete) hour — simulating
        // data that hasn't been rolled up yet (e.g. rollup failed).
        db.insert_1s(
            key,
            current_hour_start + 1000,
            Some(0),
            Some(0),
            Some(0),
            Some(42.0),
            Some(99.0),
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

        // Range > 1h and ≤ 24h → selects the 1h table.
        let since = current_hour_start - 7_200_000;
        let until = now + 1000;
        let range = until - since;
        assert!(
            range > 3_600_000 && range <= 86_400_000,
            "range must be >1h and ≤24h, got {range}"
        );

        let points = db
            .query_timeseries(key, "decode_tps", since, until)
            .await
            .unwrap();

        // Should have 2 points: the completed hour from the 1h table, and the
        // current hour from the 1s supplement.
        assert_eq!(
            points.len(),
            2,
            "should have 1h bucket + 1s current-hour supplement"
        );

        // First point: the completed hour from the 1h table.
        assert_eq!(points[0].timestamp_ms, current_hour_start - 3_600_000);
        assert!((points[0].value - 50.0).abs() < f64::EPSILON);

        // Second point: the current hour from 1s data (AVG of 99.0 = 99.0).
        assert_eq!(points[1].timestamp_ms, current_hour_start);
        assert!(
            (points[1].value - 99.0).abs() < f64::EPSILON,
            "current hour should come from 1s data, got {}",
            points[1].value
        );
    }

    /// When the 1h table has a stale row for the current hour (from a partial
    /// rollup), the query should replace it with fresh 1s data.
    #[tokio::test]
    async fn test_timeseries_1h_range_replaces_stale_current_hour() {
        let db = test_db();
        let key = "stale-engine";
        let now = chrono_now_ms();
        let current_hour_start = (now / 3_600_000) * 3_600_000;

        // Insert a stale 1h row for the CURRENT hour (partial rollup wrote
        // old data before the rollup failed).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 10.0, 5.0, 30)",
                params![key, current_hour_start],
            )
            .unwrap();
        }

        // Insert fresh 1s data for the current hour.
        db.insert_1s(
            key,
            current_hour_start + 1000,
            Some(0),
            Some(0),
            Some(0),
            Some(0.0),
            Some(77.0),
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

        let since = current_hour_start - 3_600_000;
        let until = now + 1000;

        let points = db
            .query_timeseries(key, "decode_tps", since, until)
            .await
            .unwrap();

        // The stale 1h row (value=5.0) should be replaced by fresh 1s data
        // (value=77.0), not duplicated.
        let current_hour_points: Vec<&TimeSeriesPoint> = points
            .iter()
            .filter(|p| p.timestamp_ms == current_hour_start)
            .collect();
        assert_eq!(
            current_hour_points.len(),
            1,
            "should have exactly one point for the current hour"
        );
        assert!(
            (current_hour_points[0].value - 77.0).abs() < f64::EPSILON,
            "stale 1h value should be replaced by fresh 1s data, got {}",
            current_hour_points[0].value
        );
    }

    /// When the requested range starts mid-hour, the completed-hour bucket
    /// that STRADDLES `since` must be included ONLY when raw 1s data does
    /// not already cover part of that hour inside the window (1s data was
    /// rolled up + pruned), and its timestamp must be clamped to `since`
    /// so clients with a fixed [since, until] x-domain don't expand it
    /// backwards (~2h axis on a 1h chart).
    #[tokio::test]
    async fn test_timeseries_1h_range_includes_straddle_bucket_clamped() {
        let db = test_db();
        let key = "straddle-engine";
        // Fixed epoch — fully deterministic, no wall clock. h1 must be
        // hour-aligned (divisible by 3_600_000) so the SQL floor matches.
        let h1 = 360_000_000_000i64; // previous (completed) hour bucket
        let h2 = h1 + 3_600_000; // current (incomplete) hour bucket
        let now = h2 + 1_200_000; // 20 min into the current hour
                                  // → since = now − 30 min = 50 min into h1: STRADDLE.

        // Completed previous hour in the 1h table (as the rollup would write).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 10.0, 42.0, 60)",
                params![key, h1],
            )
            .unwrap();
        }

        // Live 1s data only in the CURRENT hour (previous hour was pruned).
        db.insert_1s(
            key,
            h2 + 1000,
            Some(0),
            Some(0),
            Some(0),
            None,
            Some(99.0),
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

        // Window starts mid-PREVIOUS-hour (like the frontend's [now-60min,
        // now] when now is 30 min past the hour): `since` lands mid-h1, so
        // the completed h1 bucket STRADDLES the window start.
        let since = now - 1_800_000;
        let until = now + 1000;
        assert!(
            since > h1,
            "since must be mid-way through the previous hour (h1)"
        );

        let points = db
            .query_timeseries(key, "decode_tps", since, until)
            .await
            .unwrap();

        // 2 points: the clamped straddle bucket + the current-hour 1s point.
        assert_eq!(
            points.len(),
            2,
            "straddle bucket + current-hour point, got {points:?}"
        );
        // NO point before the window start — the axis stays 1h wide.
        assert!(
            points[0].timestamp_ms >= since,
            "straddle bucket must be clamped into the window: got {} < {since}",
            points[0].timestamp_ms
        );
        // Its value is the straddle bucket's (42.0), not the current hour's.
        assert!(
            (points[0].value - 42.0).abs() < f64::EPSILON,
            "clamped point should carry the straddle bucket's value, got {}",
            points[0].value
        );
        // The 1s point survives at its own timestamp.
        assert!(
            (points[1].value - 99.0).abs() < f64::EPSILON,
            "current-hour point should be the 1s sample, got {}",
            points[1].value
        );
    }

    /// When raw 1s data ALREADY covers part of the straddling hour inside
    /// the window (1s retention ≈ 1h ≫ typical 1m/5m seed window), the
    /// completed-hour rollup bucket must be SUPPRESSED: mixing in its
    /// hour-wide average puts an artificial value-jump point at the window's
    /// left edge of every 1m/5m/1h chart queried in the first minutes of an
    /// hour. The raw 1s rows are the truthful data for that stretch.
    #[tokio::test]
    async fn test_timeseries_straddle_bucket_suppressed_when_raw_covers_window() {
        let db = test_db();
        let key = "cover-engine";
        // Fixed epoch — fully deterministic, no wall clock.
        let h1 = 360_000_000_000i64; // previous (completed) hour bucket
        let h2 = h1 + 3_600_000; // current (incomplete) hour bucket
        let now = h2 + 1_200_000; // 20 min into the current hour

        // Completed previous hour in the 1h table with an hour-wide average
        // (42.0) that differs wildly from the raw 1s samples (99.0) — the
        // artifact source this test pins down.
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 10.0, 42.0, 60)",
                params![key, h1],
            )
            .unwrap();
        }

        // Raw 1s data in the PREVIOUS hour inside the window (un-pruned),
        // plus some in the current hour. Geometry chosen to make this test
        // FAIL under the degenerate `min_ts <= since_ms` check (the round-3
        // regression): `since` = h1+3_000_000 is deliberately UNALIGNED on
        // the query's 5002ms bucket grid (since % B = 1378), and the first
        // raw rows sit at since+3624 / since+3800 — inside the NEXT bucket
        // (bucket_start = since+3624), which is within one bucket width of
        // `since`, so the coverage check still suppresses. Because the raw
        // query clamps bucket_ts up to `since` (MAX(bucket, since)), first
        // rows in `since`'s OWN bucket would collide with the clamped
        // rollup point and dedupe would mask the artifact; the next-bucket
        // placement leaves the 42.0 visible if the check ever regresses.
        for ts in [h1 + 3_003_624, h1 + 3_003_800, h2 + 1000] {
            db.insert_1s(
                key,
                ts,
                Some(0),
                Some(0),
                Some(0),
                None,
                Some(99.0),
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
        }

        // Same geometry as the pruned case: window starts mid-h1 (10 min
        // before the boundary), straddling the h1 bucket.
        let since = now - 1_800_000;
        let until = now + 1000;
        assert!(since > h1, "window must straddle the h1/h2 boundary");

        let points = db
            .query_timeseries(key, "decode_tps", since, until)
            .await
            .unwrap();

        // The h1 rollup bucket (42.0) is GONE — raw 1s rows cover that
        // stretch. Only raw-derived points remain, none carrying 42.0.
        assert!(
            points.iter().all(|p| (p.value - 42.0).abs() > f64::EPSILON),
            "straddle rollup bucket must be suppressed when 1s data covers the stretch: {points:?}"
        );
        // The pre-boundary raw rows survive (the window's left stretch is
        // covered by real data, not an hour-wide average).
        assert!(
            points
                .iter()
                .any(|p| p.timestamp_ms >= since && p.timestamp_ms < h2),
            "raw 1s rows from the previous hour must remain, got {points:?}"
        );
        // Every point still sits inside the window (clamp contract holds).
        assert!(
            points
                .iter()
                .all(|p| p.timestamp_ms >= since && p.timestamp_ms <= until),
            "all points must stay inside [since, until], got {points:?}"
        );
    }

    /// Companion to the suppression test above: when the raw 1s rows do
    /// NOT reach back to the window's start, the straddling rollup bucket
    /// must be KEPT — it is the only source covering [since, first raw
    /// sample). The discriminating geometry vs the OLD per-bucket
    /// existence check: raw rows exist INSIDE the straddling hour but
    /// after `since` (engine restarted 5 min before the hour boundary) —
    /// the old check suppressed the bucket there (leaving a hole), the
    /// window-edge coverage check keeps it.
    #[tokio::test]
    async fn test_timeseries_straddle_bucket_kept_when_raw_misses_window_start() {
        let db = test_db();
        let key = "gap-engine";
        // Same fixed epoch geometry: window starts 10 min before the h1/h2
        // boundary; raw 1s data exists ONLY in the current hour h2.
        let h1 = 360_000_000_000i64;
        let h2 = h1 + 3_600_000;
        let now = h2 + 1_200_000; // 20 min into the current hour

        // Completed previous hour in the 1h table (hour-wide avg 42.0).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 10.0, 42.0, 60)",
                params![key, h1],
            )
            .unwrap();
        }

        // Raw 1s data starts INSIDE the straddling hour but well after the
        // window's start (since = h2 - 600_000): an engine restart 5 min
        // before the h1/h2 boundary. Under the old per-bucket check this
        // geometry suppressed the bucket (hole); the window-edge check
        // keeps it.
        for ts in [h2 - 300_000, h2 - 299_500, h2 + 1000] {
            db.insert_1s(
                key,
                ts,
                Some(0),
                Some(0),
                Some(0),
                None,
                Some(99.0),
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
        }

        let since = now - 1_800_000; // 10 min before the h1/h2 boundary
        let until = now + 1000;
        assert!(since > h1, "window must straddle the h1/h2 boundary");

        let points = db
            .query_timeseries(key, "decode_tps", since, until)
            .await
            .unwrap();

        // The straddle bucket (42.0) is KEPT: raw coverage starts 5 min
        // into the window (post-restart), so the hourly average is the
        // only coverage for the [since, h2 - 300_000) stretch. Its
        // timestamp is clamped to `since`.
        let straddle = points
            .iter()
            .find(|p| (p.value - 42.0).abs() < f64::EPSILON)
            .expect("straddle rollup bucket must be kept when raw misses window start");
        assert_eq!(
            straddle.timestamp_ms, since,
            "straddle bucket must be clamped to the window start"
        );
        // Raw points from the current hour are present alongside it. (The
        // raw bucket floor can land a hair before h2 when bucket_ms doesn't
        // divide the hour — value, not position, identifies raw points here.)
        assert!(
            points.iter().any(|p| (p.value - 99.0).abs() < f64::EPSILON),
            "raw 1s points must be present, got {points:?}"
        );
    }

    #[tokio::test]
    async fn test_timeseries_mem_used_pct_only_in_1s() {
        let db = test_db();
        let key = "ts-engine";
        let base = 1_700_000_000_000i64;

        db.insert_1s(
            key,
            base,
            Some(0),
            Some(0),
            Some(0),
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
            Some(65.0),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Small range → 1s table → should get the value.
        let points = db
            .query_timeseries(key, "mem_used_pct", base, base + 2000)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert!((points[0].value - 65.0).abs() < f64::EPSILON);

        // Large range → 1h table → mem_used_pct has no aggregate → empty.
        let points = db
            .query_timeseries(key, "mem_used_pct", 0, 7_200_000)
            .await
            .unwrap();
        assert!(points.is_empty(), "mem_used_pct has no aggregate column");
    }

    #[tokio::test]
    async fn test_timeseries_queue_time_and_tpot_1s() {
        let db = test_db();
        let key = "ts-engine";
        let base = 1_700_000_000_000i64;

        db.insert_1s(
            key,
            base,
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
            Some(3),
            Some(15.0), // queue_time_ms
            Some(25.0), // tpot_ms
            None,
        )
        .await
        .unwrap();

        // Query queue_time_ms at 1s resolution
        let points = db
            .query_timeseries(key, "queue_time_ms", base, base + 2000)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert!((points[0].value - 15.0).abs() < f64::EPSILON);

        // Query tpot_ms at 1s resolution
        let points = db
            .query_timeseries(key, "tpot_ms", base, base + 2000)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert!((points[0].value - 25.0).abs() < f64::EPSILON);

        // Query total_requests at 1s resolution
        let points = db
            .query_timeseries(key, "total_requests", base, base + 2000)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 5.0);
    }

    #[tokio::test]
    async fn test_timeseries_queue_time_and_tpot_1h_aggregation() {
        let db = test_db();
        let key = "ts-engine";
        let hour1 = (chrono_now_ms() / 3_600_000) * 3_600_000 - 7_200_000;

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
            Some(0),
            Some(10.0), // queue_time_ms
            Some(20.0), // tpot_ms
            None,
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
            Some(2),
            Some(30.0), // queue_time_ms
            Some(40.0), // tpot_ms
            None,
        )
        .await
        .unwrap();

        let _rolled = db.rollup_1s_to_1h().await.unwrap();

        // Query at 1h range → should hit 1h table with avg.
        // Range must be > 1h and ≤ 24h to select the 1h table.
        let range_end = hour1 + 7_200_000; // hour1 + 2h (well within 24h of hour1)
        let range_start = hour1 - 3_600_000; // 1h before, total range = 3h
        let points = db
            .query_timeseries(key, "queue_time_ms", range_start, range_end)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        // avg(10.0, 30.0) = 20.0
        assert!((points[0].value - 20.0).abs() < f64::EPSILON);

        let points = db
            .query_timeseries(key, "tpot_ms", range_start, range_end)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        // avg(20.0, 40.0) = 30.0
        assert!((points[0].value - 30.0).abs() < f64::EPSILON);

        // total_requests in 1h table should be sum
        let points = db
            .query_timeseries(key, "total_requests", range_start, range_end)
            .await
            .unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 5.0, "sum of 2+3 = 5");
    }

    // -----------------------------------------------------------------
    // rollup latency column tests
    // -----------------------------------------------------------------

    /// Insert 1s rows with latency values, run rollup, and verify the 1h
    /// table has non-NULL latency columns (ttft_ms_p95, itl_ms_p95,
    /// e2e_ms_p95, queue_time_ms_avg, tpot_ms_avg).
    #[tokio::test]
    async fn test_rollup_1s_to_1h_populates_latency_columns() {
        let db = test_db();
        let key = "lat-engine";
        // Two samples in the same completed hour, 3 hours ago.
        let hour = (chrono_now_ms() / 3_600_000) * 3_600_000 - 10_800_000;

        for (i, (ttft, itl, e2e, qt, tpot)) in [
            (100.0, 10.0, 500.0, 5.0, 30.0),
            (200.0, 20.0, 600.0, 15.0, 40.0),
        ]
        .iter()
        .enumerate()
        {
            db.insert_1s(
                key,
                hour + 1000 * (i as i64 + 1),
                Some(50),
                Some(100),
                Some(2),
                Some(40.0),
                Some(25.0),
                Some(*ttft),
                Some(*itl),
                Some(*e2e),
                Some(100.0),
                Some(70.0),
                Some(40.0),
                Some(2),
                Some(0),
                Some(0.5),
                Some(0.05),
                Some(55.0),
                Some(45.0),
                Some(0),
                Some(*qt),
                Some(*tpot),
                None,
            )
            .await
            .unwrap();
        }

        let rolled = db.rollup_1s_to_1h().await.unwrap();
        assert!(rolled >= 1, "should roll up at least 1 hour bucket");

        // Read the latency columns directly from snapshots_1h.
        let conn = db.inner.lock().await;
        let (ttft, itl, e2e, qt, tpot) = read_latency(&conn, "snapshots_1h", key, None);
        drop(conn);

        // AVG(100, 200) = 150
        assert!(ttft.is_some(), "ttft_ms_p95 should not be NULL");
        assert!((ttft.unwrap() - 150.0).abs() < f64::EPSILON, "ttft avg");
        // AVG(10, 20) = 15
        assert!(itl.is_some(), "itl_ms_p95 should not be NULL");
        assert!((itl.unwrap() - 15.0).abs() < f64::EPSILON, "itl avg");
        // AVG(500, 600) = 550
        assert!(e2e.is_some(), "e2e_ms_p95 should not be NULL");
        assert!((e2e.unwrap() - 550.0).abs() < f64::EPSILON, "e2e avg");
        // AVG(5, 15) = 10
        assert!(qt.is_some(), "queue_time_ms_avg should not be NULL");
        assert!((qt.unwrap() - 10.0).abs() < f64::EPSILON, "queue_time avg");
        // AVG(30, 40) = 35
        assert!(tpot.is_some(), "tpot_ms_avg should not be NULL");
        assert!((tpot.unwrap() - 35.0).abs() < f64::EPSILON, "tpot avg");
    }

    /// Verify ON CONFLICT DO UPDATE updates existing latency rows. Insert a
    /// 1h row with NULL latency, run rollup with latency data, and confirm
    /// the row is backfilled rather than ignored.
    #[tokio::test]
    async fn test_rollup_1s_to_1h_updates_existing_latency() {
        let db = test_db();
        let key = "upd-engine";
        let hour = (chrono_now_ms() / 3_600_000) * 3_600_000 - 10_800_000;

        // Pre-insert an hourly row with NULL latency columns (simulating a
        // row created by the old INSERT OR IGNORE rollup).
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, total_prompt_tokens, total_gen_tokens, \
                  total_requests, prompt_tps_avg, decode_tps_avg, sample_count) \
                 VALUES (?1, ?2, 0, 0, 0, 0.0, 0.0, 1)",
                params![key, hour],
            )
            .unwrap();
            // Confirm it's NULL before rollup.
            let qt: Option<f64> = conn
                .query_row(
                    "SELECT queue_time_ms_avg FROM snapshots_1h WHERE engine_key=?1 AND bucket_ts=?2",
                    params![key, hour],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(qt.is_none(), "precondition: latency should be NULL");
        }

        // Insert 1s latency data for the same hour.
        db.insert_1s(
            key,
            hour + 1000,
            Some(10),
            Some(20),
            Some(1),
            Some(40.0),
            Some(25.0),
            Some(88.0),
            Some(8.0),
            Some(400.0),
            Some(100.0),
            Some(70.0),
            Some(40.0),
            Some(2),
            Some(0),
            Some(0.5),
            Some(0.05),
            Some(55.0),
            Some(45.0),
            Some(0),
            Some(12.0),
            Some(33.0),
            None,
        )
        .await
        .unwrap();

        let rolled = db.rollup_1s_to_1h().await.unwrap();
        // ON CONFLICT DO UPDATE counts as 1 affected row in SQLite.
        assert!(rolled >= 1, "rollup should update the existing row");

        let conn = db.inner.lock().await;
        let (ttft, itl, e2e, qt, tpot) = read_latency(&conn, "snapshots_1h", key, Some(hour));
        drop(conn);

        assert!(ttft.is_some() && (ttft.unwrap() - 88.0).abs() < f64::EPSILON);
        assert!(itl.is_some() && (itl.unwrap() - 8.0).abs() < f64::EPSILON);
        assert!(e2e.is_some() && (e2e.unwrap() - 400.0).abs() < f64::EPSILON);
        assert!(qt.is_some() && (qt.unwrap() - 12.0).abs() < f64::EPSILON);
        assert!(tpot.is_some() && (tpot.unwrap() - 33.0).abs() < f64::EPSILON);
    }

    /// Verify rollup_1h_to_1d populates latency columns from the 1h table and
    /// uses ON CONFLICT DO UPDATE for existing daily rows.
    #[tokio::test]
    async fn test_rollup_1h_to_1d_populates_and_updates_latency() {
        let db = test_db();
        let key = "day-engine";
        // Use a fixed day 2 days ago so it's a "complete" day.
        let now = chrono_now_ms();
        let day_start = (now / 86_400_000) * 86_400_000 - 86_400_000; // yesterday start
        let hour1 = day_start + 3_600_000;
        let hour2 = day_start + 7_200_000;

        {
            let conn = db.inner.lock().await;
            // Insert two hourly rows with latency values.
            for (bucket, ttft, itl, e2e, qt, tpot) in [
                (hour1, 100.0, 10.0, 500.0, 5.0, 30.0),
                (hour2, 200.0, 20.0, 600.0, 15.0, 40.0),
            ] {
                conn.execute(
                    "INSERT INTO snapshots_1h \
                     (engine_key, bucket_ts, total_prompt_tokens, total_gen_tokens, \
                      total_requests, prompt_tps_avg, decode_tps_avg, sample_count, \
                      ttft_ms_p95, itl_ms_p95, e2e_ms_p95, queue_time_ms_avg, tpot_ms_avg) \
                     VALUES (?1, ?2, 10, 20, 1, 40.0, 25.0, 60, ?3, ?4, ?5, ?6, ?7)",
                    params![key, bucket, ttft, itl, e2e, qt, tpot],
                )
                .unwrap();
            }
        }

        let rolled = db.rollup_1h_to_1d(None).await.unwrap();
        assert!(rolled >= 1, "should roll up at least 1 day bucket");

        // Read latency columns from snapshots_1d.
        let conn = db.inner.lock().await;
        let (ttft, itl, e2e, qt, tpot) = read_latency(&conn, "snapshots_1d", key, None);
        drop(conn);

        // AVG(100, 200) = 150
        assert!(
            ttft.is_some() && (ttft.unwrap() - 150.0).abs() < f64::EPSILON,
            "1d ttft avg"
        );
        assert!(
            itl.is_some() && (itl.unwrap() - 15.0).abs() < f64::EPSILON,
            "1d itl avg"
        );
        assert!(
            e2e.is_some() && (e2e.unwrap() - 550.0).abs() < f64::EPSILON,
            "1d e2e avg"
        );
        assert!(
            qt.is_some() && (qt.unwrap() - 10.0).abs() < f64::EPSILON,
            "1d queue avg"
        );
        assert!(
            tpot.is_some() && (tpot.unwrap() - 35.0).abs() < f64::EPSILON,
            "1d tpot avg"
        );

        // Now update the hourly data with new latency values and re-rollup.
        // ON CONFLICT DO UPDATE should refresh the daily row.
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "UPDATE snapshots_1h SET ttft_ms_p95 = 999.0 \
                 WHERE engine_key = ?1 AND bucket_ts = ?2",
                params![key, hour1],
            )
            .unwrap();
        }

        db.rollup_1h_to_1d(None).await.unwrap();

        let conn = db.inner.lock().await;
        let ttft: f64 = conn
            .query_row(
                "SELECT ttft_ms_p95 FROM snapshots_1d WHERE engine_key = ?1",
                params![key],
                |r| r.get(0),
            )
            .unwrap();
        drop(conn);
        // AVG(999, 200) = 599.5
        assert!(
            (ttft - 599.5).abs() < 0.01,
            "daily ttft should be updated to AVG(999,200)=599.5, got {}",
            ttft
        );
    }

    // -----------------------------------------------------------------
    // KV cache peak correctness tests
    // -----------------------------------------------------------------

    /// Different time ranges should return different peak KV cache values when
    /// the underlying data has different maxima in those ranges.
    #[tokio::test]
    async fn test_peak_kv_cache_different_time_ranges() {
        let db = test_db();
        let key = "peak-engine";
        let now = chrono_now_ms();
        let current_hour_start = (now / 3_600_000) * 3_600_000;

        // Insert data with distinct kv_cache_pct values in different hours.
        // Hour A (2 hours ago): peak = 50.0
        let hour_a = current_hour_start - 7_200_000;
        db.insert_1s(
            key,
            hour_a + 1000,
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
            Some(50.0),
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

        // Hour B (1 hour ago): peak = 62.16
        let hour_b = current_hour_start - 3_600_000;
        db.insert_1s(
            key,
            hour_b + 1000,
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
            Some(62.16),
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

        // Roll up completed hours so they land in the 1h table.
        db.rollup_1s_to_1h().await.unwrap();

        // Range covering only hour A (from 1h table).
        let summary_a = db
            .query_summary(key, hour_a, hour_a + 3_600_000 - 1)
            .await
            .unwrap();
        assert!(summary_a.is_some());
        assert!(
            (summary_a.unwrap().peak_kv_cache_pct.unwrap() - 50.0).abs() < 0.01,
            "hour A peak should be 50.0"
        );

        // Range covering only hour B (from 1h table).
        let summary_b = db
            .query_summary(key, hour_b, hour_b + 3_600_000 - 1)
            .await
            .unwrap();
        assert!(summary_b.is_some());
        assert!(
            (summary_b.unwrap().peak_kv_cache_pct.unwrap() - 62.16).abs() < 0.01,
            "hour B peak should be 62.16"
        );

        // Range covering both hour A and hour B (from 1h table).
        let summary_ab = db
            .query_summary(key, hour_a, hour_b + 3_600_000 - 1)
            .await
            .unwrap();
        assert!(summary_ab.is_some());
        assert!(
            (summary_ab.unwrap().peak_kv_cache_pct.unwrap() - 62.16).abs() < 0.01,
            "combined A+B peak should be MAX(50.0, 62.16) = 62.16"
        );
    }

    /// The merge logic (daily + hourly) should use MAX for peak KV cache,
    /// not average or min.
    #[tokio::test]
    async fn test_merge_peak_kv_cache_uses_max() {
        let db = test_db();
        let key = "merge-peak-engine";
        let now = chrono_now_ms();
        let start_of_today = (now / 86_400_000) * 86_400_000;

        // Insert a daily row for yesterday with kv_cache_pct_max = 70.0.
        let yesterday = start_of_today - 86_400_000;
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, kv_cache_pct_max, kv_cache_pct_avg, sample_count) \
                 VALUES (?1, ?2, 100, 70.0, 50.0, 86400)",
                params![key, yesterday],
            )
            .unwrap();
        }

        // Insert an hourly row for today with kv_cache_pct_max = 55.0.
        let today_hour = start_of_today + 3_600_000;
        {
            let conn = db.inner.lock().await;
            conn.execute(
                "INSERT INTO snapshots_1h \
                 (engine_key, bucket_ts, total_prompt_tokens, kv_cache_pct_max, kv_cache_pct_avg, sample_count) \
                 VALUES (?1, ?2, 50, 55.0, 40.0, 3600)",
                params![key, today_hour],
            )
            .unwrap();
        }

        // Query spanning yesterday + today → triggers merge path.
        let summary = db.query_summary(key, yesterday, now + 1000).await.unwrap();
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(
            s.source_table, "mixed",
            "should use the merge path (daily + hourly)"
        );
        assert!(
            (s.peak_kv_cache_pct.unwrap() - 70.0).abs() < 0.01,
            "merged peak should be MAX(70.0, 55.0) = 70.0"
        );
    }

    /// FIX 1: hours spanning local midnight roll into the LOCAL day.
    #[tokio::test]
    async fn test_rollup_1h_to_1d_local_day_boundary() {
        let db = test_db();
        let key = "tz-engine";
        let ofs_h: i64 = -4; // ET
                             // Buckets at UTC-midnight+0..3h == ET 20:00-23:00 of PREVIOUS day.
        let midnight = (chrono_now_ms() / 86_400_000) * 86_400_000 - 86_400_000;
        {
            let c = db.inner.lock().await;
            for h in 0..4 {
                c.execute(
                    "INSERT INTO snapshots_1h (engine_key,bucket_ts,total_prompt_tokens,sample_count) VALUES (?1,?2,?3,3600)",
                    rusqlite::params![key, midnight + h * 3_600_000, 1000 + h],
                )
                .unwrap();
            }
        }
        let ofs_ms = ofs_h * 3_600_000;
        let rolled = db.rollup_1h_to_1d(Some(ofs_ms)).await.unwrap();
        assert_eq!(rolled, 1, "all four hours = ONE local day");
        let (bts, sum): (i64, i64) = {
            let c = db.inner.lock().await;
            c.query_row(
                "SELECT bucket_ts,total_prompt_tokens FROM snapshots_1d WHERE engine_key=?1",
                rusqlite::params![key],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap()
        };
        // LOCAL-day key must satisfy: ((t - ofs)floor)+ofs == bts for every
        // member bucket. Members: midnight..midnight+3h; with ofs=-4h their
        // (t-ofs) = midnight+4..7h → same UTC-day => key = midnight+4h-4h??
        // EXACT: ((t +4h)/86400)*86400 -4h. For t=UTC-midnight: shifted noon
        // → dayfloor=UTC-midnight → key=UTC-midnight-4h = PREV 20:00 ET. All
        // four hours share ONE day because shifted=4..7h <24h. => bts:
        // mirrors SQL exactly: ((t - ofs) / 86400000)*86400000 + ofs
        let any_t = midnight; // UTC midnight
        let want = ((any_t - ofs_ms) / 86_400_000) * 86_400_000 + ofs_ms;
        assert_eq!(bts, want, "daily bucket = LOCAL-midnight key");
        assert_eq!(sum, 4000 + 6, "token sums preserved across boundary");
    }

    /// FIX 2: migrate() backfills legacy NULL sample_count/power.
    #[tokio::test]
    async fn test_migrate_backfills_null_seconds_and_power() {
        let db = test_db();
        {
            let c = db.inner.lock().await;
            // drop+recreate WITHOUT the backfill? simpler: insert row with
            // NULLs directly (schema allows).
            c.execute(
                "INSERT INTO snapshots_1d (engine_key,bucket_ts,total_prompt_tokens,sample_count,power_watts_sum) VALUES ('e',1783728000000,5000,86400,NULL)",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO snapshots_1h (engine_key,bucket_ts,total_prompt_tokens,sample_count) VALUES ('e2',1783728000000,77,3600)",
                [],
            )
            .unwrap();
            c.execute(
                "UPDATE snapshots_1h SET power_watts_sum=NULL WHERE engine_key='e2'",
                [],
            )
            .unwrap();
            HistoryDb::migrate(&c).unwrap(); // idempotent: runs backfill
            let (cnt, pw): (i64, f64) = c
                .query_row(
                    "SELECT sample_count,COALESCE(power_watts_sum,-1) FROM snapshots_1d WHERE engine_key='e'",
                    [],
                    |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
                )
                .unwrap();
            assert_eq!(cnt, 86400, "legacy-NULL power row keeps its seconds");
            assert_eq!(pw, 0.0);
            let hcnt: i64 = c
                .query_row(
                    "SELECT sample_count FROM snapshots_1h WHERE engine_key='e2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(hcnt, 3600);
        }
    }

    /// Verifies that `merge_summaries` correctly picks the higher peak
    /// regardless of which side (daily or hourly) has the larger value.
    #[test]
    fn test_merge_summaries_peak_max_logic() {
        let daily = HistorySummary {
            delta_prompt_tokens: 100,
            delta_gen_tokens: 200,
            total_requests: 10,
            avg_decode_tps: 30.0,
            avg_prompt_tps: 50.0,
            peak_active_requests: 5,
            peak_queued_requests: 2,
            peak_kv_cache_pct: Some(60.0),
            avg_kv_cache_pct: Some(40.0),
            total_preemptions: Some(0),
            power_kwh: 1.0,
            total_seconds: Some(3600.0),
            source_table: "daily",
        };
        let hourly = HistorySummary {
            delta_prompt_tokens: 50,
            delta_gen_tokens: 100,
            total_requests: 5,
            avg_decode_tps: 25.0,
            avg_prompt_tps: 45.0,
            peak_active_requests: 8,
            peak_queued_requests: 3,
            peak_kv_cache_pct: Some(75.0),
            avg_kv_cache_pct: Some(50.0),
            total_preemptions: Some(2),
            power_kwh: 0.5,
            total_seconds: Some(1800.0),
            source_table: "hourly",
        };

        let merged = merge_summaries(&daily, &hourly);
        assert!(
            (merged.peak_kv_cache_pct.unwrap() - 75.0).abs() < 0.01,
            "peak should be MAX(60.0, 75.0) = 75.0"
        );
        assert_eq!(
            merged.peak_active_requests, 8,
            "peak active should be MAX(5, 8) = 8"
        );
        assert_eq!(
            merged.peak_queued_requests, 3,
            "peak queued should be MAX(2, 3) = 3"
        );
        assert_eq!(merged.source_table, "mixed");
    }

    // -----------------------------------------------------------------
    // Daily rollup lookback + duplicate-bucket dedupe
    // -----------------------------------------------------------------

    /// Defect 1 regression: a daily row older than the lookback window must
    /// NOT be rewritten by rollup_1h_to_1d even when its hourly source rows
    /// have been partially pruned. Before the fix, the rollup re-aggregated
    /// ALL hourly rows and overwrote old daily totals with the shrinking
    /// remainder (the shrinking "All time" cost-avoided bug).
    #[tokio::test]
    async fn test_rollup_1h_to_1d_does_not_rewrite_days_outside_lookback() {
        let db = test_db();
        let key = "old-engine";
        let now = chrono_now_ms();
        let current_day_start = (now / 86_400_000) * 86_400_000;
        // 10 days ago: far outside the 3-day lookback, inside the 30-day
        // hourly retention so this is exactly the live-DB failure window.
        let old_day = current_day_start - 10 * 86_400_000;

        {
            let c = db.inner.lock().await;
            // Four hourly source rows for the old day...
            for h in 0..4 {
                c.execute(
                    "INSERT INTO snapshots_1h \
                     (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                     VALUES (?1, ?2, ?3, 3600)",
                    params![key, old_day + h * 3_600_000, 1000 + h],
                )
                .unwrap();
            }
            // ...the daily row holding the ORIGINAL full-day totals, and a
            // simulated pruning that leaves only the first hour standing.
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 8640000, 86400)",
                params![key, old_day],
            )
            .unwrap();
            c.execute(
                "DELETE FROM snapshots_1h WHERE engine_key = ?1 AND bucket_ts >= ?2",
                params![key, old_day + 3_600_000],
            )
            .unwrap();
        }

        db.rollup_1h_to_1d(None).await.unwrap();

        let (tokens, sc): (i64, i64) = {
            let c = db.inner.lock().await;
            c.query_row(
                "SELECT total_prompt_tokens, sample_count FROM snapshots_1d \
                 WHERE engine_key = ?1 AND bucket_ts = ?2",
                params![key, old_day],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap()
        };
        assert_eq!(
            tokens, 8640000,
            "old daily row must not be rewritten from pruned hourly sources"
        );
        assert_eq!(sc, 86400, "old daily sample_count must stay intact");
    }

    /// Companion to the lookback regression: a daily row WITHIN the lookback
    /// window is still re-aggregated normally from its hourly sources.
    #[tokio::test]
    async fn test_rollup_1h_to_1d_updates_recent_day_within_lookback() {
        let db = test_db();
        let key = "recent-engine";
        let now = chrono_now_ms();
        let current_day_start = (now / 86_400_000) * 86_400_000;
        let yesterday = current_day_start - 86_400_000;

        {
            let c = db.inner.lock().await;
            // Stale daily row that must be refreshed...
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 1, 1)",
                params![key, yesterday],
            )
            .unwrap();
            // ...and four hourly source rows totalling 2000 tokens / 14400s.
            for h in 0..4 {
                c.execute(
                    "INSERT INTO snapshots_1h \
                     (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                     VALUES (?1, ?2, 500, 3600)",
                    params![key, yesterday + h * 3_600_000],
                )
                .unwrap();
            }
        }

        db.rollup_1h_to_1d(None).await.unwrap();

        let (tokens, sc): (i64, i64) = {
            let c = db.inner.lock().await;
            c.query_row(
                "SELECT total_prompt_tokens, sample_count FROM snapshots_1d \
                 WHERE engine_key = ?1 AND bucket_ts = ?2",
                params![key, yesterday],
                |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())),
            )
            .unwrap()
        };
        assert_eq!(tokens, 2000, "recent day re-aggregated from hourly rows");
        assert_eq!(sc, 14400, "recent day sample_count = SUM(4 x 3600)");
    }

    /// Defect 2 regression: duplicate daily buckets for the same shifted day
    /// (legacy UTC-keyed + offset-keyed) collapse to one row; the survivor is
    /// the row with the larger sample_count (tie-break: larger
    /// total_prompt_tokens); running the rollup again is a no-op. Rows of
    /// other engines sharing the same calendar day are left untouched.
    #[tokio::test]
    async fn test_rollup_1h_to_1d_dedupes_duplicate_day_buckets() {
        let db = test_db();
        db.set_setting("utc_offset", "-4").await.unwrap();
        let key = "dup-engine";
        let guard_key = "guard-engine";
        let tie_key = "tie-engine";
        let now = chrono_now_ms();
        let current_day_start = (now / 86_400_000) * 86_400_000;
        let utc_day = current_day_start - 5 * 86_400_000;
        let ofs_ms = -4 * 3_600_000;
        // Same day-bucket formula the rollup/dedupe uses: a legacy UTC-midnight
        // row and the offset-keyed row both map to this shifted-day key.
        let shifted_key = ((utc_day - ofs_ms) / 86_400_000) * 86_400_000 + ofs_ms;
        assert_ne!(
            shifted_key, utc_day,
            "offset-keyed row differs from UTC key"
        );

        {
            let c = db.inner.lock().await;
            // Duplicate pair for `key`: legacy UTC-keyed row (full day) vs
            // offset-keyed row (degraded by the pruned-source rewrite bug).
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 5000, 86400)",
                params![key, utc_day],
            )
            .unwrap();
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 100, 3600)",
                params![key, shifted_key],
            )
            .unwrap();
            // Same-day row of another engine: single row, must survive.
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 777, 86400)",
                params![guard_key, utc_day],
            )
            .unwrap();
            // Tie on sample_count: larger total_prompt_tokens must win.
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 700, 86400)",
                params![tie_key, utc_day],
            )
            .unwrap();
            c.execute(
                "INSERT INTO snapshots_1d \
                 (engine_key, bucket_ts, total_prompt_tokens, sample_count) \
                 VALUES (?1, ?2, 300, 86400)",
                params![tie_key, shifted_key],
            )
            .unwrap();
        }

        db.rollup_1h_to_1d(None).await.unwrap();

        let read_rows = |c: &Connection, engine: &str| -> Vec<(i64, i64, i64)> {
            let mut stmt = c
                .prepare(
                    "SELECT bucket_ts, total_prompt_tokens, sample_count \
                     FROM snapshots_1d WHERE engine_key = ?1",
                )
                .unwrap();
            stmt.query_map(params![engine], |r| {
                Ok((r.get(0).unwrap(), r.get(1).unwrap(), r.get(2).unwrap()))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
        };

        {
            let c = db.inner.lock().await;
            let rows = read_rows(&c, key);
            assert_eq!(rows.len(), 1, "duplicate pair collapses to one row");
            assert_eq!(rows[0].0, utc_day, "survivor keeps the larger-sample row");
            assert_eq!(rows[0].1, 5000);
            assert_eq!(rows[0].2, 86400);

            let guard = read_rows(&c, guard_key);
            assert_eq!(guard.len(), 1, "single-row day is untouched");
            assert_eq!(guard[0].1, 777);

            let tie = read_rows(&c, tie_key);
            assert_eq!(tie.len(), 1, "tied pair collapses to one row");
            assert_eq!(tie[0].1, 700, "tie-break keeps more prompt tokens");
        }

        // Idempotent: a second rollup tick changes nothing further.
        db.rollup_1h_to_1d(None).await.unwrap();
        {
            let c = db.inner.lock().await;
            assert_eq!(read_rows(&c, key).len(), 1, "dedupe is a no-op on rerun");
            assert_eq!(read_rows(&c, guard_key).len(), 1);
            assert_eq!(read_rows(&c, tie_key).len(), 1);
        }
    }
}
