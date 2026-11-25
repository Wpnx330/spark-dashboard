# decision.md — Command tab (OPCODE-TASK-COMMAND-TAB.md)

Locked decisions D1–D5 honored; deviations and judgment calls logged here.

## Root decisions honored

- **D1 — third tab `Command`**: Done. Header now reads
  `Command | Metrics | Historical`; `Command` sits first (leftmost) matching
  the "am I good?" glance-first flow. Default tab remains `metrics`
  (byte-identical landing behavior for existing users).
- **D2 — layout**: Done. Row 1 = two cluster cards in
  `grid-cols-1 md:grid-cols-2`; Row 2 = single full-width savings band with
  per-cluster $ + GRAND TOTAL; time-slice dropdown (Live/24h/7d/30d/1y) top
  right on md+, driving the whole page from one state. Card titles come from
  `/api/meta` (local and peer) — nothing hardcoded.
- **D3 — zero new backend state**: Done. Local: existing `/ws` for Live,
  existing `/api/history/summary` + `/api/history/timeseries` for ranges.
  Peer: frontend fetches `<peer>/api/meta`, `<peer>/api/history/*` directly
  over the LAN. No proxy, no new DB, no new endpoints, no new deps.
  `SPARK_DASHBOARD_PEER_URL` unset → peer card renders dimmed with
  "Peer not configured (SPARK_DASHBOARD_PEER_URL)"; everything else unchanged.
- **D4 — mobile**: Done. Cards stack to one column; savings band is single
  column; the window selector moves to a full-width row ABOVE the cards
  (`flex-col md:flex-row` control row); smallest text is `text-[10px]`
  labels with `text-xs`+ values; truncation used instead of clipping.
- **D5 — live correctness**: Done. Live savings = same cost math
  (`lib/savings.ts`, single source shared with HistoryView) over the
  TODAY window (local midnight from the dashboard's configured
  `utc_offset` — same algorithm as HistoryView's `today` preset, pinned by
  an explicit equality test in `commandWindow.test.ts`).

## Deviations / judgment calls

1. **HistoryView.tsx touched (safety refactor, behavior-identical).**
   Constraint 2 says "Metrics and Historical tabs untouched". D2 also says
   "Reuse the existing electricity-rate + API-equivalent pricing math from
   HistoryView — do not fork the logic." Those conflict unless the math is
   extracted: keeping one implementation requires moving it out of
   HistoryView. The extraction (`lib/savings.ts`) is behavior-preserving —
   reviewed line-by-line, all pre-existing HistoryView tests pass unmodified.
   This is the only change to an existing tab file.
2. **`/api/meta` extended additively** (`peer_url`, `engine_key`, `model`).
   Needed because (a) the peer's title must come from ITS `/api/meta`, and
   (b) `/api/history/summary` requires an `engine_key` that HistoryView
   normally derives from the live WS — which the peer's WS is not visible to
   this host. Exposing the primary engine's endpoint + model name via the
   existing meta shape is the constraint-4 "additive touch"; no new endpoint,
   no state, reads the already-shared engine snapshot RwLock.
3. **Peer "Live" mode approximates instantaneous values with a rolling
   60s summary + 5s poll** instead of a cross-origin WebSocket. The task
   scoped Live to "existing WS (/ws) for Live" for the LOCAL cluster; a
   cross-origin WS to the peer would be new surface (and the peer may be an
   older binary). A 60s aggregate of active/queued peaks + rates is the
   closest honest read-only equivalent over plain GETs.
4. **Peer TTFT-p95 / GPU-temp come from the `ttft_ms` / `gpu_temp`
   timeseries max over the window** (aggregates), and from the local
   snapshot's `ttft_percentiles.p95` / GPU metrics in Live. These match the
   condensed-line spec (prefill tok/s, decode tok/s, TTFT p95, total reqs)
   with the fields the summary API already exposes.
5. **Grand-total fallback**: when a peer is configured but unreachable, its
   savings row shows `—` and is excluded from the grand total (counted as 0)
   rather than failing the whole band.
6. **Engine-key fallback chain** (peer meta `engine_key` → local engine
   endpoint → `vllm`): peers running an older binary (pre-this-commit) do
   not publish `engine_key`; both clusters serve vLLM locally so the local
   endpoint is the right first guess. Verified against the live cluster-2
   dashboard during QA.
