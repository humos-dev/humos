import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * WereAwayBanner. Shows a one-line summary of pipe fires and signal
 * broadcasts that happened while the app was closed for more than 5 minutes.
 *
 * Reuses the .update-banner CSS to stay visually consistent with the existing
 * coordination-tinted strip. Suppresses zero-count segments so the banner
 * never reads "0 pipes fired, 0 signals broadcast".
 */

const LAST_OPEN_KEY = "humos-last-open";
const GAP_MS = 5 * 60 * 1000;

interface EventLogEntry {
  id: number;
  ts: string;
  event_type: string;
  message: string;
  success: boolean;
  payload_tokens: number;
  source_tokens: number;
  success_ids: string;
  fail_ids: string;
}

interface HealthState {
  status: string; // "ok" | "init_failed" | "queue_saturated" | "uninitialized"
}

interface Summary {
  pipeCount: number;
  pipeTokens: number;
  signalCount: number;
  failCount: number;
}

function summarize(events: EventLogEntry[]): Summary {
  let pipeCount = 0;
  let pipeTokens = 0;
  let signalCount = 0;
  let failCount = 0;

  for (const e of events) {
    if (e.event_type === "pipe") {
      pipeCount += 1;
      pipeTokens += e.payload_tokens || 0;
    } else if (e.event_type === "signal") {
      signalCount += 1;
    }
    if (!e.success) {
      failCount += 1;
    }
  }

  return { pipeCount, pipeTokens, signalCount, failCount };
}

function hasAnything(s: Summary): boolean {
  return s.pipeCount > 0 || s.signalCount > 0 || s.failCount > 0;
}

export function WereAwayBanner() {
  const [summary, setSummary] = useState<Summary | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [healthDegraded, setHealthDegraded] = useState<string | null>(null);

  // Poll event log health on mount and surface a chip if degraded. Init
  // failures and queue saturation are silent in the Rust layer; this is the
  // user's only visible signal that the activity history is incomplete.
  useEffect(() => {
    let cancelled = false;
    invoke<HealthState>("event_log_health")
      .then((h) => {
        if (cancelled) return;
        if (h.status === "init_failed" || h.status === "queue_saturated") {
          setHealthDegraded(h.status);
        }
      })
      .catch(() => {
        // ignore: command might not exist in older builds
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    // Capture the prior timestamp BEFORE deciding to write a new one.
    // React strict mode double-mounts in dev: if we wrote on the first mount
    // unconditionally, the second mount would read its own write and
    // suppress the banner. Defer the write until after the gap check passes
    // (or until invoke resolves) so a remount during fetch keeps reading the
    // original timestamp.
    let last: string | null = null;
    try {
      last = localStorage.getItem(LAST_OPEN_KEY);
    } catch {
      // Private mode / quota errors. Fall through to "show if events exist".
    }

    let lastMs = NaN;
    if (last != null) {
      lastMs = Date.parse(last);
    }

    // Treat negative gap as "show if there are events". This handles a future
    // timestamp from clock skew during the prior session and a malformed value
    // (NaN). Both states should not silently suppress real coordination.
    const gap = Number.isNaN(lastMs) ? Infinity : Date.now() - lastMs;
    const shouldQuery = gap < 0 || gap > GAP_MS;

    let cancelled = false;
    if (shouldQuery) {
      invoke<EventLogEntry[]>("list_event_log", { limit: 20 })
        .then((events) => {
          if (cancelled) return;
          const s = summarize(events);
          if (hasAnything(s)) {
            setSummary(s);
          }
          // Only stamp the new timestamp after the query resolves. A strict-mode
          // double-mount that beat us here will still see the original `last`.
          try {
            localStorage.setItem(LAST_OPEN_KEY, new Date().toISOString());
          } catch {
            // ignore quota errors
          }
        })
        .catch((e) => {
          // Fire-and-forget. A missing event log should never break app mount.
          console.warn("WereAwayBanner: list_event_log failed", e);
          try {
            localStorage.setItem(LAST_OPEN_KEY, new Date().toISOString());
          } catch {
            // ignore
          }
        });
    } else {
      // Gap was small (recent session): refresh the timestamp so future
      // mounts measure from now.
      try {
        localStorage.setItem(LAST_OPEN_KEY, new Date().toISOString());
      } catch {
        // ignore
      }
    }

    return () => {
      cancelled = true;
    };
  }, []);

  if (dismissed) {
    return null;
  }

  // If only the health chip needs to show (no summary), render a minimal
  // version of the banner with just the warning text.
  if (summary == null) {
    if (healthDegraded == null) {
      return null;
    }
    const healthCopy =
      healthDegraded === "init_failed"
        ? "Activity history disabled (event log could not initialize)."
        : "Activity history degraded (event log queue saturated).";
    return (
      <div className="update-banner" style={{ position: "relative" }}>
        <div className="update-banner__left">
          <span className="update-banner__arrow">&#x21BA;</span>
          <span style={{ color: "var(--error)" }}>{healthCopy}</span>
        </div>
        <div className="update-banner__right">
          <button
            className="update-banner__dismiss"
            onClick={() => setDismissed(true)}
            aria-label="Dismiss event log warning"
          >
            &#xD7;
          </button>
        </div>
      </div>
    );
  }

  const segments: string[] = [];
  if (summary.pipeCount > 0) {
    const tokenLabel = summary.pipeTokens > 0
      ? `${summary.pipeCount} ${summary.pipeCount === 1 ? "pipe" : "pipes"} fired (${summary.pipeTokens.toLocaleString()} tokens)`
      : `${summary.pipeCount} ${summary.pipeCount === 1 ? "pipe" : "pipes"} fired`;
    segments.push(tokenLabel);
  }
  if (summary.signalCount > 0) {
    segments.push(
      `${summary.signalCount} ${summary.signalCount === 1 ? "signal broadcast" : "signals broadcast"}`,
    );
  }

  const summaryLine = segments.length > 0
    ? `While you were away: ${segments.join(", ")}.`
    : "While you were away.";

  return (
    <div className="update-banner" style={{ position: "relative" }}>
      <div className="update-banner__left">
        <span className="update-banner__arrow">&#x21BA;</span>
        <span>{summaryLine}</span>
        {summary.failCount > 0 && (
          <span style={{ color: "var(--error)", marginLeft: "8px" }}>
            {summary.failCount} {summary.failCount === 1 ? "event" : "events"} failed.
          </span>
        )}
        {healthDegraded != null && (
          <span style={{ color: "var(--error)", marginLeft: "8px" }}>
            History {healthDegraded === "init_failed" ? "disabled" : "degraded"}.
          </span>
        )}
      </div>
      <div className="update-banner__right">
        <button
          className="update-banner__dismiss"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss were-away banner"
        >
          &#xD7;
        </button>
      </div>
    </div>
  );
}
