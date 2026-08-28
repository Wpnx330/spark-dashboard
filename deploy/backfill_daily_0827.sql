-- One-time backfill: rebuild daily rows from hourly buckets, keyed to LOCAL
-- day (utc_offset). Applying commit 06a25a3 semantics to existing data.
-- SAFE: idempotent (ON CONFLICT updates). Backup exists: history.db.bak-0827.
-- Run: ssh DGX1 'sqlite3 /var/lib/spark-dashboard/history.db < backfill_daily_0827.sql'

--1. resolve offset (default -4 = ET)
INSERT OR REPLACE INTO settings(key,value) VALUES('backfill_0827','started');

--2. rebuild every LOCAL-day daily row from hourlies (full history)
INSERT INTO snapshots_1d
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
WITH
 h AS (
   SELECT engine_key,
          (( bucket_ts - ( (-4) * 3600000 ) )
            / 86400000) * 86400000
            + ( (-4) * 3600000 ) AS lday,
          SUM(COALESCE(total_prompt_tokens,0)) tp,
          SUM(COALESCE(total_gen_tokens,0)) tg,
          SUM(COALESCE(total_requests,0)) tr,
          AVG(prompt_tps_avg) pa, MAX(prompt_tps_max) px,
          AVG(decode_tps_avg) da, MAX(decode_tps_max) dx,
          AVG(ttft_ms_p95) tt, AVG(itl_ms_p95) it, AVG(e2e_ms_p95) ee,
          COALESCE(SUM(power_watts_sum),0) pw,
          AVG(gpu_util_avg) gu, AVG(gpu_temp_avg) gt, MAX(gpu_temp_max) gx,
          MAX(active_requests_max) am, MAX(queued_requests_max) qm,
          AVG(kv_cache_pct_avg) ka, MAX(kv_cache_pct_max) kk,
          AVG(prefix_cache_hit_avg) ph,
          AVG(cpu_util_avg) cu, SUM(sample_count) sc, MAX(preemptions_total) pr
   FROM snapshots_1h
   GROUP BY engine_key, lday
 )
SELECT engine_key, lday, tp, tg, tr, pa,px, da,dx, tt,it,ee, pw,
       gu,gt,gx, am,qm, ka,kk,ph, cu, sc, pr,
       NULL, NULL, NULL
FROM h
WHERE TRUE
ON CONFLICT(engine_key,bucket_ts) DO UPDATE SET
  total_prompt_tokens=excluded.total_prompt_tokens,
  total_gen_tokens=excluded.total_gen_tokens,
  total_requests=excluded.total_requests,
  prompt_tps_avg=excluded.prompt_tps_avg,
  prompt_tps_max=excluded.prompt_tps_max,
  decode_tps_avg=excluded.decode_tps_avg,
  decode_tps_max=excluded.decode_tps_max,
  ttft_ms_p95=excluded.ttft_ms_p95,
  itl_ms_p95=excluded.itl_ms_p95,
  e2e_ms_p95=excluded.e2e_ms_p95,
  power_watts_sum=excluded.power_watts_sum,
  gpu_util_avg=excluded.gpu_util_avg,
  gpu_temp_avg=excluded.gpu_temp_avg,
  gpu_temp_max=excluded.gpu_temp_max,
  active_requests_max=excluded.active_requests_max,
  queued_requests_max=excluded.queued_requests_max,
  kv_cache_pct_avg=excluded.kv_cache_pct_avg,
  kv_cache_pct_max=excluded.kv_cache_pct_max,
  prefix_cache_hit_avg=excluded.prefix_cache_hit_avg,
  cpu_util_avg=excluded.cpu_util_avg,
  sample_count=excluded.sample_count,
  preemptions_total=excluded.preemptions_total;

--3. backfill bookkeeping
INSERT OR REPLACE INTO settings(key,value) VALUES('backfill_0827','done');

--4. verify (prints)
SELECT ' Aug26-ET', SUM(total_prompt_tokens) FROM (
  SELECT ( (bucket_ts - (-4*3600000)) / 86400000)*86400000 + (-4*3600000) d,
         total_prompt_tokens FROM snapshots_1h)
WHERE d = ((strftime('%s','2026-08-26 00:00:00','-4 hours')*1000/86400000)*86400000 - 4*3600000);
