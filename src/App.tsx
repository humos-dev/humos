import { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SessionCard } from "./SessionCard";
import { BrainRibbon } from "./BrainRibbon";
import { UpdateBanner } from "./UpdateBanner";
import { DaemonLoginBanner } from "./DaemonLoginBanner";
import { WereAwayBanner } from "./WereAwayBanner";
import { useVersionCheck } from "./hooks/useVersionCheck";
import { useRelatedContexts } from "./hooks/useRelatedContexts";
import { PipeConfig } from "./PipeConfig";
import type { SessionState, PipeTokenState } from "./types";
import { colors, spacing, fontSize, radius } from "./tokens";

interface DaemonHealth {
  online: boolean;
  index_sessions: number;
  uptime_secs: number;
}

interface SignalFiredEvent {
  message: string;
  success_ids: string[];
  fail_ids: string[];
  success_count: number;
  fail_count: number;
}

interface PipeFiredEvent {
  rule_id: string;
  from_session_id: string;
  to_session_id: string;
  message: string;
  success: boolean;
  error?: string;
  payload_tokens: number;
}

interface PipeRule {
  id: string;
  from_session_id: string;
  to_session_id: string;
  from_cwd: string;
  to_cwd: string;
  trigger: { OnIdle: null } | { OnFileWrite: string };
}

interface LogEntry {
  id: number;
  text: string;
  ts: string;
  isError?: boolean;
}

const LOG_KEY = "humos-activity-log";
const LOG_MAX = 20;

function now(): string {
  return new Date().toLocaleTimeString("en-US", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

const LOG_VERSION = "2";

function loadStoredLog(): LogEntry[] {
  try {
    // Clear stale logs from older versions
    if (localStorage.getItem("humos-log-version") !== LOG_VERSION) {
      localStorage.removeItem(LOG_KEY);
      localStorage.setItem("humos-log-version", LOG_VERSION);
      return [];
    }
    const raw = localStorage.getItem(LOG_KEY);
    return raw ? (JSON.parse(raw) as LogEntry[]) : [];
  } catch {
    return [];
  }
}

function animatePipeLine(
  canvas: HTMLCanvasElement,
  fromEl: HTMLElement,
  toEl: HTMLElement,
  drawBackground?: () => void,
): () => void {
  const ctx = canvas.getContext("2d");
  if (ctx == null) return () => {};
  const c: CanvasRenderingContext2D = ctx;

  const fr = fromEl.getBoundingClientRect();
  const tr = toEl.getBoundingClientRect();

  const x1 = fr.left + fr.width / 2;
  const y1 = fr.top + fr.height / 2;
  const x2 = tr.left + tr.width / 2;
  const y2 = tr.top + tr.height / 2;

  const start = performance.now();
  const duration = 500;
  let rafId = 0;
  let cancelled = false;
  let timeoutA: ReturnType<typeof setTimeout> | null = null;
  let timeoutB: ReturnType<typeof setTimeout> | null = null;

  function draw(now: number) {
    if (cancelled) return;
    const t = Math.min((now - start) / duration, 1);
    const ease = 1 - Math.pow(1 - t, 3);

    c.clearRect(0, 0, canvas.width, canvas.height);
    drawBackground?.();

    c.beginPath();
    c.moveTo(x1, y1);
    c.lineTo(x1 + (x2 - x1) * ease, y1 + (y2 - y1) * ease);
    c.strokeStyle = "#3b82f6";
    c.lineWidth = 1.5;
    c.setLineDash([4, 4]);
    c.stroke();

    const headX = x1 + (x2 - x1) * ease;
    const headY = y1 + (y2 - y1) * ease;
    c.beginPath();
    c.arc(headX, headY, 3, 0, Math.PI * 2);
    c.fillStyle = "#3b82f6";
    c.fill();

    if (t < 1) {
      rafId = requestAnimationFrame(draw);
    } else {
      toEl.style.transition = "box-shadow 0s";
      toEl.style.boxShadow = "0 0 0 1px #3b82f6";
      timeoutA = setTimeout(() => {
        if (cancelled) return;
        toEl.style.transition = "box-shadow 0.6s ease";
        toEl.style.boxShadow = "";
        timeoutB = setTimeout(() => {
          if (cancelled) return;
          toEl.style.transition = "";
          c.clearRect(0, 0, canvas.width, canvas.height);
          drawBackground?.();
        }, 600);
      }, 150);
    }
  }

  rafId = requestAnimationFrame(draw);
  return () => {
    cancelled = true;
    cancelAnimationFrame(rafId);
    if (timeoutA !== null) clearTimeout(timeoutA);
    if (timeoutB !== null) clearTimeout(timeoutB);
  };
}

function drawPipeEdges(
  canvas: HTMLCanvasElement | null,
  sessions: SessionState[],
  pipeRules: PipeRule[],
): void {
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  // Always clear first so deleted rules don't leave ghost edges.
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (pipeRules.length === 0) return;

  for (const rule of pipeRules) {
    const fromEl = document.querySelector<HTMLElement>(`[data-session-id="${rule.from_session_id}"]`);
    const toEl = document.querySelector<HTMLElement>(`[data-session-id="${rule.to_session_id}"]`);
    if (!fromEl || !toEl) continue;

    const fr = fromEl.getBoundingClientRect();
    const tr = toEl.getBoundingClientRect();
    if (!fr.width || !tr.width) continue;

    const fromSession = sessions.find((s) => s.id === rule.from_session_id);
    const toSession = sessions.find((s) => s.id === rule.to_session_id);
    const bothIdle = fromSession?.status === "idle" && toSession?.status === "idle";

    const x1 = fr.left + fr.width / 2;
    const y1 = fr.top + fr.height / 2;
    const x2 = tr.left + tr.width / 2;
    const y2 = tr.top + tr.height / 2;

    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    if (bothIdle) {
      ctx.strokeStyle = "rgba(100, 100, 100, 0.25)";
      ctx.lineWidth = 1;
      ctx.setLineDash([4, 6]);
    } else {
      ctx.strokeStyle = "rgba(59, 130, 246, 0.35)";
      ctx.lineWidth = 1;
      ctx.setLineDash([]);
    }
    ctx.stroke();
    ctx.setLineDash([]);

    if (!bothIdle) {
      const angle = Math.atan2(y2 - y1, x2 - x1);
      const headLen = 7;
      ctx.beginPath();
      ctx.moveTo(x2, y2);
      ctx.lineTo(x2 - headLen * Math.cos(angle - Math.PI / 6), y2 - headLen * Math.sin(angle - Math.PI / 6));
      ctx.moveTo(x2, y2);
      ctx.lineTo(x2 - headLen * Math.cos(angle + Math.PI / 6), y2 - headLen * Math.sin(angle + Math.PI / 6));
      ctx.strokeStyle = "rgba(59, 130, 246, 0.35)";
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  }
}

const styles: Record<string, React.CSSProperties> = {
  app: {
    minHeight: "100vh",
    background: colors.bg,
    display: "flex",
    flexDirection: "column",
    paddingBottom: "28px",
  },
  header: {
    padding: `${spacing.md}px ${spacing.lg}px`,
    borderBottom: `1px solid ${colors.border}`,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    background: colors.bg2,
  },
  headerLeft: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  title: {
    fontSize: fontSize.xl,
    fontWeight: 600,
    color: colors.text,
    letterSpacing: "0.02em",
  },
  subtitle: {
    fontSize: fontSize.sm,
    color: colors.text2,
    marginTop: "2px",
  },
  headerRight: {
    display: "flex",
    alignItems: "center",
    gap: `${spacing.sm}px`,
  },
  badge: {
    background: colors.border,
    border: `1px solid ${colors.border}`,
    borderRadius: radius.sm,
    padding: `${spacing.xs}px 10px`,
    fontSize: fontSize.sm,
    color: colors.text2,
  },
  pipeBtn: {
    background: "transparent",
    border: `1px solid ${colors.border}`,
    borderRadius: radius.sm,
    padding: `${spacing.xs}px 10px`,
    fontSize: fontSize.sm,
    color: colors.text2,
    cursor: "pointer",
    fontFamily: "inherit",
    letterSpacing: "0.04em",
    transition: "border-color 0.15s, color 0.15s",
  },
  pipeBtnActive: {
    borderColor: colors.signal,
    color: colors.signal,
  },
  main: {
    flex: 1,
    padding: `${spacing.lg}px`,
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(340px, 1fr))",
    alignItems: "start",
    gap: `${spacing.md}px`,
  },
  empty: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    height: "60vh",
    color: colors.text3,
    fontSize: fontSize.lg,
    gap: `${spacing.sm}px`,
  },
};

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [loading, setLoading] = useState(true);
  const [daemonOnline, setDaemonOnline] = useState<boolean | null>(null);
  const { newVersion, releaseUrl } = useVersionCheck();
  const [updateBannerDismissed, setUpdateBannerDismissed] = useState(false);
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  const [viewMode, setViewMode] = useState<"grid" | "list">(() =>
    (localStorage.getItem("humos-view") as "grid" | "list") ?? "grid"
  );
  const [pipeOpen, setPipeOpen] = useState(false);
  const [pipeRules, setPipeRules] = useState<PipeRule[]>([]);
  const initialLog = useState<LogEntry[]>(loadStoredLog)[0];
  const [log, setLog] = useState<LogEntry[]>(initialLog);
  const [signalOpen, setSignalOpen] = useState(false);
  const [signalMessage, setSignalMessage] = useState("");
  const [signalPending, setSignalPending] = useState(false);
  const [signalConfirmPending, setSignalConfirmPending] = useState(false);
  const [signalFlashIds, setSignalFlashIds] = useState<Set<string>>(new Set());
  const [signalFailIds, setSignalFailIds] = useState<Set<string>>(new Set());
  const [signalError, setSignalError] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const drawEdgesRef = useRef<() => void>(() => {});
  const [pipeHistory, setPipeHistory] = useState<Map<string, { fromProject: string; ts: number }>>(new Map());
  // Last-fire token snapshot per pipe rule. Wired via the pipe-fired listener.
  // payload_tokens comes from the Rust event; source_tokens is summed from the
  // source session's input + output tokens at fire time.
  const [pipeTokens, setPipeTokens] = useState<Map<string, PipeTokenState>>(new Map());
  // Monotonic log entry counter. Initialized to one past the highest id in
  // the persisted log so new entries never collide with restored ones.
  // Without this, useRef(0) reset on every app launch and new pipe/signal
  // events were assigned ids that already existed in localStorage, producing
  // React's "two children with the same key" warning and risking dropped
  // entries in the activity log render.
  const logSeqRef = useRef(
    initialLog.reduce((m, e) => Math.max(m, e.id), -1) + 1
  );
  // Mirror of sessions for use inside event listener closures without staleness.
  const sessionsRef = useRef<SessionState[]>([]);
  const signalUndoRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const signalFlashTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const signalFailTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const signalInputRef = useRef<HTMLInputElement>(null);

  const loadSessions = useCallback(async () => {
    try {
      const result = await invoke<SessionState[]>("get_sessions");
      setSessions(result);
      sessionsRef.current = result;
    } catch (err) {
      console.error("Failed to load sessions:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadPipeRules = useCallback(async () => {
    try {
      const result = await invoke<PipeRule[]>("list_pipe_rules");
      setPipeRules(result);
    } catch (err) {
      console.error("Failed to load pipe rules:", err);
    }
  }, []);

  const checkDaemonHealth = useCallback(async () => {
    try {
      const health = await invoke<DaemonHealth>("check_daemon_health");
      setDaemonOnline(health.online);
    } catch {
      setDaemonOnline(false);
    }
  }, []);

  useEffect(() => {
    loadSessions();
    loadPipeRules();
    checkDaemonHealth();

    const sessionInterval = setInterval(loadSessions, 5_000);
    const healthInterval = setInterval(checkDaemonHealth, 5_000);
    return () => {
      clearInterval(sessionInterval);
      clearInterval(healthInterval);
    };
  }, [loadSessions, loadPipeRules, checkDaemonHealth]);

  // Project Brain: bulk-fetch related contexts for all visible cwds
  const visibleCwds = useMemo(
    () => sessions.map((s) => s.cwd).filter(Boolean),
    [sessions],
  );
  const relatedContexts = useRelatedContexts(visibleCwds, daemonOnline);

  // Suppress ribbon when all sessions share 1 cwd (no distinguishing info)
  const uniqueCwdCount = useMemo(() => new Set(visibleCwds).size, [visibleCwds]);

  // Dense grid: >3 cards have ribbon data
  const ribbonCount = useMemo(() => {
    let count = 0;
    for (const s of sessions) {
      const ctx = relatedContexts.get(s.cwd);
      if (ctx && ctx.daemon_online && ctx.matches.length > 0 && !dismissed.has(s.id)) {
        count++;
      }
    }
    return count;
  }, [sessions, relatedContexts, dismissed]);

  // Persist activity log to localStorage on every change.
  // Error entries are shown in-session but NOT persisted - they're transient.
  useEffect(() => {
    try {
      const persistable = log.filter((e) => !e.isError).slice(0, LOG_MAX);
      localStorage.setItem(LOG_KEY, JSON.stringify(persistable));
    } catch {
      // localStorage unavailable in some Tauri configs - silent fail.
    }
  }, [log]);

  // Listen for pipe-fired events - animate and log.
  useEffect(() => {
    let cancelAnim: (() => void) | null = null;

    const unlisten = listen<PipeFiredEvent>("pipe-fired", (event) => {
      const { rule_id, from_session_id, to_session_id, success, error, payload_tokens } = event.payload;

      // Use the ref so this closure is never stale.
      const current = sessionsRef.current;
      const fromSession = current.find((s) => s.id === from_session_id);
      const fromName = fromSession?.project ?? from_session_id.slice(0, 8);
      const toName =
        current.find((s) => s.id === to_session_id)?.project ??
        to_session_id.slice(0, 8);

      // Snapshot the source session's cumulative tokens at fire time so the
      // savings ratio reflects what the source had built up vs what the pipe
      // forwarded. Update on BOTH success and failure so the badge never
      // shows stale success-only data on a rule that's been failing.
      // opencode sessions report 0 for both fields, in which case PipeConfig
      // renders the "tokens: n/a" fallback.
      const sourceTokens =
        (fromSession?.input_tokens ?? 0) + (fromSession?.output_tokens ?? 0);
      setPipeTokens((prev) => {
        const next = new Map(prev);
        next.set(rule_id, {
          payload_tokens: payload_tokens ?? 0,
          source_tokens: sourceTokens,
          success,
        });
        return next;
      });

      // Only animate the pipe line on successful injection.
      if (success) {
        setPipeHistory((prev) => {
          const next = new Map(prev);
          next.set(to_session_id, { fromProject: fromName, ts: Date.now() });
          return next;
        });

        const canvas = canvasRef.current;
        if (canvas) {
          const fromEl = document.querySelector<HTMLElement>(
            `[data-session-id="${from_session_id}"]`
          );
          const toEl = document.querySelector<HTMLElement>(
            `[data-session-id="${to_session_id}"]`
          );
          if (fromEl && toEl) {
            cancelAnim?.();
            cancelAnim = animatePipeLine(canvas, fromEl, toEl, () => drawEdgesRef.current());
          }
        }
      }

      setLog((prev) => {
        // Format: "⌁ pipe: api → tests · 180 tokens" on success.
        // payload_tokens === 0 (or undefined) means no token data, e.g. opencode
        // source. Render "tokens: n/a" instead of "0 tokens".
        // Clamp display at 1B tokens so a corrupted/wild number can't blow out
        // the activity log row width. Beyond 1B render "1B+ tokens".
        const TOKEN_DISPLAY_CAP = 1_000_000_000;
        let tokenSuffix: string;
        if (!payload_tokens || payload_tokens <= 0) {
          tokenSuffix = "tokens: n/a";
        } else if (payload_tokens > TOKEN_DISPLAY_CAP) {
          tokenSuffix = "1B+ tokens";
        } else {
          tokenSuffix = `${payload_tokens.toLocaleString()} tokens`;
        }
        const text = success
          ? `⌁ pipe: ${fromName} → ${toName} · ${tokenSuffix}`
          : `pipe failed: ${fromName} → ${toName}${error ? ` (${error.slice(0, 60)})` : ""}`;
        const entry: LogEntry = {
          id: logSeqRef.current++,
          text,
          ts: now(),
        };
        return [entry, ...prev].slice(0, LOG_MAX);
      });
    });

    return () => {
      cancelAnim?.();
      unlisten.then((f) => f());
    };
  }, []); // stable - reads sessionsRef, not sessions state

  // Resize canvas to window and redraw edges (setting width/height clears canvas).
  useEffect(() => {
    function resize() {
      const c = canvasRef.current;
      if (!c) return;
      c.width = window.innerWidth;
      c.height = window.innerHeight;
      drawEdgesRef.current();
    }
    resize();
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);

  // Keep drawEdgesRef current and redraw after layout changes (50ms lets
  // the grid re-render before we measure card positions via getBoundingClientRect).
  useEffect(() => {
    drawEdgesRef.current = () => drawPipeEdges(canvasRef.current, sessions, pipeRules);
    const t = setTimeout(() => drawEdgesRef.current(), 50);
    return () => clearTimeout(t);
  }, [sessions, pipeRules]);

  // Reload rules whenever pipe drawer opens or closes.
  useEffect(() => {
    loadPipeRules();
  }, [pipeOpen, loadPipeRules]);

  // Clear any pending signal-related timeouts on unmount so we don't fire
  // invoke() or setState against a torn-down component. Mirrors the v0.3.4
  // animatePipeLine cleanup pattern for the flash/fail timers that previously
  // leaked and could clobber a newer signal's flash set mid-flight.
  useEffect(() => {
    return () => {
      if (signalUndoRef.current) {
        clearTimeout(signalUndoRef.current);
        signalUndoRef.current = null;
      }
      if (signalFlashTimeoutRef.current) {
        clearTimeout(signalFlashTimeoutRef.current);
        signalFlashTimeoutRef.current = null;
      }
      if (signalFailTimeoutRef.current) {
        clearTimeout(signalFailTimeoutRef.current);
        signalFailTimeoutRef.current = null;
      }
    };
  }, []);

  // Close only the currently-open modal with Escape. Signal bar takes priority
  // if both are somehow open - but the mutual-exclusion logic on toggle should
  // prevent that.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (signalOpen) {
        handleSignalCancel();
      } else if (pipeOpen) {
        setPipeOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [signalOpen, pipeOpen]);

  // Focus the signal input whenever the command bar opens (covers re-open,
  // which won't re-run autoFocus).
  useEffect(() => {
    if (signalOpen && signalInputRef.current) {
      signalInputRef.current.focus();
    }
  }, [signalOpen]);

  // Listen for signal-fired events - flash cards and log.
  useEffect(() => {
    const unlisten = listen<SignalFiredEvent>("signal-fired", (event) => {
      const { success_ids, fail_ids, message, success_count, fail_count } = event.payload;

      if (success_ids.length > 0) {
        setSignalFlashIds(new Set(success_ids));
        // Clear any prior in-flight flash timeout so a rapid re-signal can't
        // blow away the new flash set mid-window.
        if (signalFlashTimeoutRef.current) clearTimeout(signalFlashTimeoutRef.current);
        signalFlashTimeoutRef.current = setTimeout(() => {
          setSignalFlashIds(new Set());
          signalFlashTimeoutRef.current = null;
        }, 800);
      }
      if (fail_ids.length > 0) {
        setSignalFailIds(new Set(fail_ids));
        if (signalFailTimeoutRef.current) clearTimeout(signalFailTimeoutRef.current);
        signalFailTimeoutRef.current = setTimeout(() => {
          setSignalFailIds(new Set());
          signalFailTimeoutRef.current = null;
        }, 1000);
      }

      const preview = message.length > 40 ? message.slice(0, 40) + "…" : message;
      // Suppress log entry when zero sessions received the message - a failure-only
      // entry is emitted from handleSignalSubmit instead.
      if (success_count > 0) {
        const failNote = fail_count > 0 ? ` (${fail_count} failed)` : "";
        setLog((prev) => {
          const entry: LogEntry = {
            id: logSeqRef.current++,
            text: `⌁ ${success_count}/${success_count + fail_count} · ${preview}${failNote}`,
            ts: now(),
          };
          return [entry, ...prev].slice(0, LOG_MAX);
        });
      }
    });

    return () => { unlisten.then((f) => f()); };
  }, []);

  function handleSignalSubmit() {
    const msg = signalMessage.trim();
    if (!msg || signalPending) return;

    // Confirmation gate: broadcasting to more than 3 sessions requires an
    // explicit second Enter press. This prevents accidental mass-broadcasts.
    if (nonIdleCount > 3 && !signalConfirmPending) {
      setSignalConfirmPending(true);
      return;
    }
    setSignalConfirmPending(false);

    // Hard guard against stacked undo timers: if one is already queued,
    // cancel it before starting a new one.
    if (signalUndoRef.current) {
      clearTimeout(signalUndoRef.current);
      signalUndoRef.current = null;
    }

    setSignalPending(true);
    setSignalError(null);

    signalUndoRef.current = setTimeout(async () => {
      signalUndoRef.current = null;
      try {
        const results = await invoke<Array<{ session_id: string; project: string; success: boolean; error?: string }>>(
          "signal_sessions",
          { message: msg }
        );

        // Empty-targets: shouldn't happen (button is disabled), but if the
        // session set went idle between click and fire, the backend returns
        // Err("No active sessions") which lands in the catch branch below.
        // If we somehow got an empty array back, treat as failure too.
        if (results.length === 0) {
          setSignalError("No active sessions received the message.");
          setSignalPending(false);
          return;
        }

        const failed = results.filter((r) => !r.success);
        if (failed.length === results.length) {
          // All failed: list failed project names so the user can act.
          const names = failed.map((r) => r.project).slice(0, 5).join(", ");
          const more = failed.length > 5 ? ` +${failed.length - 5} more` : "";
          setSignalError(`0 of ${results.length} received. Failed: ${names}${more}`);
          setSignalPending(false);
        } else if (failed.length > 0) {
          // Partial failure: surface names but allow the bar to close so the user
          // can see the card flashes. They still get the failure info via log + toast.
          const names = failed.map((r) => r.project).slice(0, 3).join(", ");
          const more = failed.length > 3 ? ` +${failed.length - 3}` : "";
          setLog((prev) => {
            const entry: LogEntry = {
              id: logSeqRef.current++,
              text: `⌁ signal failed for ${failed.length}: ${names}${more}`,
              isError: true,
              ts: now(),
            };
            return [entry, ...prev].slice(0, LOG_MAX);
          });
          setSignalMessage("");
          setSignalOpen(false);
          setSignalPending(false);
          setSignalError(null);
        } else {
          // All succeeded.
          setSignalMessage("");
          setSignalOpen(false);
          setSignalPending(false);
          setSignalError(null);
        }
      } catch (err) {
        // Rust returns Err for empty msg, over-length, or empty session set.
        // Strip the Tauri error prefix noise if present.
        const errMsg = String(err).replace(/^Error: /, "");
        setSignalError(errMsg);
        setSignalPending(false);
      }
    }, 2000);
  }

  function handleSignalCancel() {
    if (signalUndoRef.current) {
      clearTimeout(signalUndoRef.current);
      signalUndoRef.current = null;
    }
    setSignalPending(false);
    setSignalConfirmPending(false);
    setSignalOpen(false);
    setSignalMessage("");
    setSignalError(null);
  }

  const runningCount = sessions.filter((s) => s.status === "running").length;
  const nonIdleCount = sessions.filter((s) => s.status !== "idle").length;
  const isActive = runningCount > 0;

  // Pre-compute source/target sets for O(1) card lookups.
  const sourceIds = new Set(pipeRules.map((r) => r.from_session_id));
  const targetIds = new Set(pipeRules.map((r) => r.to_session_id));

  return (
    <div style={styles.app}>
      {/* Canvas overlay for pipe fire animation */}
      <canvas ref={canvasRef} className="pipe-fire-canvas" />

      <header style={styles.header}>
        <div style={styles.headerLeft}>
          <div className={`waveform${isActive ? " waveform--active" : ""}`}>
            <div className="waveform__bar" />
            <div className="waveform__bar" />
            <div className="waveform__bar" />
          </div>
          <div>
            <div style={styles.title}>humOS</div>
            <div style={styles.subtitle}>
              {loading
                ? "Scanning..."
                : `${sessions.length} session${sessions.length !== 1 ? "s" : ""} detected`}
            </div>
          </div>
        </div>
        <div style={styles.headerRight}>
          <div style={styles.badge}>
            {runningCount > 0 ? `${runningCount} running` : "Idle"}
          </div>
          <div className="view-toggle">
            <button
              className={`view-toggle__btn${viewMode === "list" ? " view-toggle__btn--active" : ""}`}
              onClick={() => { setViewMode("list"); localStorage.setItem("humos-view", "list"); }}
              aria-label="List view"
            >≡ List</button>
            <button
              className={`view-toggle__btn${viewMode === "grid" ? " view-toggle__btn--active" : ""}`}
              onClick={() => { setViewMode("grid"); localStorage.setItem("humos-view", "grid"); }}
              aria-label="Grid view"
            >⊞ Grid</button>
          </div>
          <button
            style={{
              ...styles.pipeBtn,
              ...(pipeOpen ? styles.pipeBtnActive : {}),
            }}
            onClick={() => {
              // Mutual exclusion: opening Pipes closes the Signal bar.
              if (signalOpen) handleSignalCancel();
              setPipeOpen((v) => !v);
            }}
          >
            Pipes {pipeOpen ? "▼" : "▲"}
            {pipeRules.length > 0 && (
              <span
                style={{
                  marginLeft: "5px",
                  background: "var(--coord)",
                  color: "#000",
                  borderRadius: "6px",
                  padding: "0 5px",
                  fontSize: "10px",
                  fontWeight: 700,
                }}
              >
                {pipeRules.length}
              </span>
            )}
          </button>
          <button
            aria-label={nonIdleCount === 0 ? "Signal (no active sessions)" : `Signal ${nonIdleCount} active session${nonIdleCount !== 1 ? "s" : ""}`}
            style={{
              ...styles.pipeBtn,
              ...(signalOpen ? styles.pipeBtnActive : {}),
              ...(nonIdleCount === 0 ? { opacity: 0.35, cursor: "not-allowed", filter: "grayscale(1)" } : {}),
            }}
            title={nonIdleCount === 0 ? "No active sessions to signal" : `Signal ${nonIdleCount} active session${nonIdleCount !== 1 ? "s" : ""}`}
            onClick={() => {
              if (nonIdleCount === 0) return;
              // Mutual exclusion: opening Signal closes the Pipes drawer.
              if (pipeOpen) setPipeOpen(false);
              setSignalOpen((v) => !v);
              setSignalError(null);
            }}
            disabled={nonIdleCount === 0}
          >
            ⌁ Signal
          </button>
        </div>
      </header>

      {/* Update banner - shown when a newer version is available */}
      {newVersion && releaseUrl && !updateBannerDismissed && (
        <UpdateBanner
          version={newVersion}
          releaseUrl={releaseUrl}
          onDismiss={() => setUpdateBannerDismissed(true)}
        />
      )}

      {/* One-time banner shown after LaunchAgent is installed on first launch */}
      <DaemonLoginBanner />

      {/* Were-away banner shows after a 5+ minute gap with new coordination events */}
      <WereAwayBanner />

      {signalOpen && (
        <div
          className={`signal-command-bar${signalError ? " signal-command-bar--error" : ""}`}
          style={{ position: "relative" }}
        >
          <input
            ref={signalInputRef}
            className="signal-command-bar__input"
            placeholder={`Signal ${nonIdleCount} session${nonIdleCount !== 1 ? "s" : ""} (e.g. "stop", "checkpoint", "re-read config"). Enter to send`}
            aria-label="Signal broadcast message"
            value={signalMessage}
            maxLength={512}
            autoFocus
            readOnly={signalPending}
            onChange={(e) => {
              setSignalMessage(e.target.value);
              setSignalError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSignalSubmit();
              }
              if (e.key === "Escape") handleSignalCancel();
            }}
          />
          {signalMessage.length > 350 && (
            <span className={`signal-command-bar__counter${signalMessage.length > 460 ? " signal-command-bar__counter--warn" : ""}`}>
              {signalMessage.length}/512
            </span>
          )}
          {signalConfirmPending && (
            <span className="signal-command-bar__toast" style={{ color: "var(--warn, #f59e0b)" }}>
              Broadcasting to {nonIdleCount} sessions. Enter to confirm,{" "}
              <button className="signal-command-bar__cancel" onClick={handleSignalCancel}>
                Esc to cancel
              </button>
            </span>
          )}
          {signalPending && (
            <span className="signal-command-bar__toast">
              Queued for {nonIdleCount} session{nonIdleCount !== 1 ? "s" : ""} -{" "}
              <button
                className="signal-command-bar__cancel"
                onClick={handleSignalCancel}
              >
                Undo
              </button>
            </span>
          )}
          {signalError && (
            <span className="signal-command-bar__error-text" role="alert">{signalError}</span>
          )}
          {/* Screen-reader live region for delivery status - visually hidden */}
          <span
            role="status"
            aria-live="polite"
            style={{
              position: "absolute",
              width: 1,
              height: 1,
              padding: 0,
              overflow: "hidden",
              clip: "rect(0, 0, 0, 0)",
              whiteSpace: "nowrap",
              border: 0,
            }}
          >
            {signalPending
              ? `Queued for ${nonIdleCount} sessions, undo available for two seconds.`
              : signalError
              ? signalError
              : ""}
          </span>
        </div>
      )}

      <main style={styles.main} className="app-grid-bg">
        {!loading && sessions.length === 0 ? (
          <div className="onboarding">
            <div className="onboarding__card">
              <h1 className="onboarding__heading">Welcome to humOS</h1>

              <div className="onboarding__steps">
                <div className="onboarding__step">
                  <span className="onboarding__step-number">1</span>
                  <div className="onboarding__step-body">
                    <div className="onboarding__step-title">Start a Claude session in Terminal</div>
                    <code className="onboarding__code">claude</code>
                  </div>
                </div>

                <div className="onboarding__step">
                  <span className="onboarding__step-number">2</span>
                  <div className="onboarding__step-body">
                    <div className="onboarding__step-title">Your session appears here automatically</div>
                    <div className="onboarding__skeleton" />
                  </div>
                </div>

                <div className="onboarding__step">
                  <span className="onboarding__step-number">3</span>
                  <div className="onboarding__step-body">
                    <div className="onboarding__step-title">Connect sessions with pipe, or broadcast with signal</div>
                  </div>
                </div>
              </div>

              <div className="onboarding__status">
                <span className="onboarding__status-dot" />
                Watching ~/.claude/projects/ for sessions…
              </div>
            </div>
          </div>
        ) : viewMode === "list" ? (
          <div className="session-list-view">
            <div className="session-list__header">
              <div className="session-list__hcell session-list__hcell--name">Session</div>
              <div className="session-list__hcell session-list__hcell--status">Status</div>
              <div className="session-list__hcell session-list__hcell--output">Last Output</div>
              <div className="session-list__hcell session-list__hcell--pipe">Pipe</div>
              <div className="session-list__hcell session-list__hcell--ts">Time</div>
              <div className="session-list__hcell session-list__hcell--actions">Actions</div>
            </div>
            {sessions.map((session) => (
              <SessionCard
                key={session.id}
                session={session}
                isSource={sourceIds.has(session.id)}
                isTarget={targetIds.has(session.id)}
                signalSuccess={signalFlashIds.has(session.id)}
                signalFail={signalFailIds.has(session.id)}
                pipeHistory={pipeHistory.get(session.id)}
                viewMode="list"
              />
            ))}
          </div>
        ) : (
          <div
            style={styles.grid}
            className={ribbonCount > 3 ? "session-grid--dense" : ""}
          >
            {sessions.map((session) => {
              const ctx = relatedContexts.get(session.cwd) ?? null;
              const showRibbon = uniqueCwdCount > 1 && ctx && ctx.daemon_online && ctx.matches.length > 0 && !dismissed.has(session.id);
              return (
                <SessionCard
                  key={session.id}
                  session={session}
                  isSource={sourceIds.has(session.id)}
                  isTarget={targetIds.has(session.id)}
                  signalSuccess={signalFlashIds.has(session.id)}
                  signalFail={signalFailIds.has(session.id)}
                  pipeHistory={pipeHistory.get(session.id)}
                  ribbon={showRibbon ? (
                    <BrainRibbon
                      context={ctx}
                      dismissed={dismissed.has(session.id)}
                      onDismiss={() => setDismissed((prev) => new Set(prev).add(session.id))}
                      onSessionFocus={(sessionId, cwd) => invoke("focus_session", { sessionId, cwd }).catch((err) => console.error("[humOS] ribbon focus failed:", err))}
                    />
                  ) : (uniqueCwdCount > 1 && daemonOnline === true && ctx === null && session.cwd ? (
                    <BrainRibbon context={null} dismissed={false} onDismiss={() => {}} />
                  ) : undefined)}
                />
              );
            })}
          </div>
        )}
      </main>

      {/* Activity log */}
      {log.length > 0 && (
        <div className="activity-log">
          {log.slice(0, 8).map((entry, i) => (
            <span
              key={`${entry.id}-${entry.ts}`}
              className="activity-log__entry"
              style={{ opacity: Math.max(0.15, 1 - i * 0.12) }}
            >
              <span style={{ color: "#333", marginRight: "6px" }}>
                {entry.ts}
              </span>
              <span className="activity-log__signal">▸</span> {entry.text}
              {i < Math.min(log.length, 8) - 1 && (
                <span style={{ color: "#222", margin: "0 10px" }}>·</span>
              )}
            </span>
          ))}
        </div>
      )}

      {/* Pipe config bottom drawer */}
      {pipeOpen && (
        <>
          <div
            className="pipe-config__overlay"
            onMouseDown={() => setPipeOpen(false)}
          />
          <PipeConfig
            sessions={sessions}
            rules={pipeRules}
            tokens={pipeTokens}
            onRulesChanged={loadPipeRules}
            onClose={() => setPipeOpen(false)}
          />
        </>
      )}
    </div>
  );
}
