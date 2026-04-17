import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface RelatedSession {
  session_id: string;
  project: string;
  cwd: string;
  snippet: string;
  modified_at: string;
}

export interface RelatedContext {
  cwd: string;
  matches: RelatedSession[];
  total_count: number;
  is_stale: boolean;
  daemon_online: boolean;
}

// Rust backend shape — mapped to RelatedContext
interface RibbonResult {
  daemon_online: boolean;
  is_stale: boolean;
  entries: {
    session_id: string;
    project: string;
    cwd: string;
    snippet: string;
    modified_at: string;
  }[];
  total_count: number;
}

/**
 * Bulk-fetch related contexts for all visible cwds. Deduplicates by cwd
 * so N cards sharing the same directory produce 1 IPC call, not N.
 * Re-fetches every `intervalMs` (default 30s) to pick up new sessions.
 */
export function useRelatedContexts(
  cwds: string[],
  daemonOnline: boolean | null,
  intervalMs = 30_000,
): Map<string, RelatedContext> {
  const [contexts, setContexts] = useState<Map<string, RelatedContext>>(new Map());
  const fetchingRef = useRef(false);

  const fetchAll = useCallback(async () => {
    if (daemonOnline !== true || fetchingRef.current) return;
    fetchingRef.current = true;

    const uniqueCwds = [...new Set(cwds.filter(Boolean))];
    const results = new Map<string, RelatedContext>();

    // Fetch in parallel, one call per unique cwd
    const promises = uniqueCwds.map(async (cwd) => {
      try {
        const result = await invoke<RibbonResult>("get_related_context", { cwd });
        results.set(cwd, {
          cwd,
          matches: result.entries.map((e) => ({
            session_id: e.session_id,
            project: e.project,
            cwd: e.cwd,
            snippet: e.snippet,
            modified_at: e.modified_at,
          })),
          total_count: result.total_count,
          is_stale: result.is_stale,
          daemon_online: result.daemon_online,
        });
      } catch {
        // IPC error for this cwd, skip
      }
    });

    await Promise.all(promises);
    setContexts(results);
    fetchingRef.current = false;
  }, [cwds.join(","), daemonOnline]);

  useEffect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, intervalMs);
    return () => clearInterval(interval);
  }, [fetchAll, intervalMs]);

  return contexts;
}
