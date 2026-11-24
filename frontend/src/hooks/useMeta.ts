import { useEffect, useState } from 'react'

/** Title shown when `SPARK_DASHBOARD_TITLE` is unset or /api/meta is unreachable. */
export const DEFAULT_DASHBOARD_TITLE = 'Spark Dashboard'

/**
 * Runtime dashboard metadata, fetched once at mount from `GET /api/meta`.
 *
 * The backend serves the title from `--title` / `SPARK_DASHBOARD_TITLE` so a
 * single binary can brand multiple hosts. Starts with
 * {@link DEFAULT_DASHBOARD_TITLE} (matching the static `<title>` in
 * `index.html`) and keeps it when the API is unreachable — e.g. `npm run dev`
 * without the backend — so the shell never flashes a wrong title.
 */
export function useMeta(): { title: string } {
  const [title, setTitle] = useState<string>(DEFAULT_DASHBOARD_TITLE)

  useEffect(() => {
    let cancelled = false

    async function load() {
      try {
        const res = await fetch('/api/meta')
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        const data = await res.json()
        if (cancelled) return
        if (typeof data?.title === 'string' && data.title.trim()) {
          document.title = data.title
          setTitle(data.title)
        }
      } catch {
        // Keep the default title when /api/meta is unavailable (dev mode).
      }
    }

    load()
    return () => {
      cancelled = true
    }
  }, [])

  return { title }
}
