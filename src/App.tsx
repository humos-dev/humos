import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SessionCard } from "./SessionCard";
import { PipeConfig } from "./PipeConfig";
import type { SessionState } from "./types";

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

function loadStoredLog(): LogEntry[] {
  try {
    const raw = localStorage.getItem(LOG_KEY);
    return raw ? (JSON.parse(raw) as LogEntry[]) : [];
  } catch {
    return [];
  }
}

function animatePipeLine(
  canvas: HTMLCanvasElement,
  fromEl: HTMLElement,
  toEl: HTMLElement
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

    c.beginPath();
    c.moveTo(x1, y1);
    c.lineTo(x1 + (x2 - x1) * ease, y1 + (y2 - y1) * ease);
    c.strokeStyle = "#3ecf8e";
    c.lineWidth = 1.5;
    c.setLineDash([4, 4]);
    c.stroke();

    const headX = x1 + (x2 - x1) * ease;
    const headY = y1 + (y2 - y1) * ease;
    c.beginPath();
    c.arc(headX, headY, 3, 0, Math.PI * 2);
    c.fillStyle = "#3ecf8e";
    c.fill();

    if (t < 1) {
      rafId = requestAnimationFrame(draw);
    } else {
      toEl.style.transition = "box-shadow 0s";
      toEl.style.boxShadow = "0 0 0 1px #3ecf8e";
      timeoutA = setTimeout(() => {
        if (cancelled) return;
        toEl.style.transition = "box-shadow 0.6s ease";
        toEl.style.boxShadow = "";
        timeoutB = setTimeout(() => {
          if (cancelled) return;
          toEl.style.transition = "";
          c.clearRect(0, 0, canvas.width, canvas.height);
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

const styles: Record<string, React.CSSProperties> = {
  app: {
    minHeight: "100vh",
    background: "var(--bg)",
    display: "flex",
    flexDirection: "column",
    paddingBottom: "28px",
  },
  header: {
    padding: "16px 24px",
    borderBottom: "1px solid var(--border)",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    background: "#0d0d0d",
  },
  headerLeft: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  title: {
    fontSize: "14px",
    fontWeight: 600,
    color: "var(--text)",
    letterSpacing: "0.05em",
    textTransform: "uppercase" as const,
  },
  subtitle: {
    fontSize: "11px",
    color: "var(--text-2)",
    marginTop: "2px",
  },
  headerRight: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  badge: {
    background: "#1a1a1a",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    padding: "4px 10px",
    fontSize: "11px",
    color: "var(--text-2)",
  },
  pipeBtn: {
    background: "transparent",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    padding: "4px 10px",
    fontSize: "11px",
    color: "var(--text-2)",
    cursor: "pointer",
    fontFamily: "inherit",
    letterSpacing: "0.04em",
    transition: "border-color 0.15s, color 0.15s",
  },
  pipeBtnActive: {
    borderColor: "var(--signal)",
    color: "var(--signal)",
  },
  main: {
    flex: 1,
    padding: "24px",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(340px, 1fr))",
    alignItems: "start",
    gap: "16px",
  },
  empty: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    height: "60vh",
    color: "#333",
    fontSize: "13px",
    gap: "8px",
  },
};

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [loading, setLoading] = useState(true);
  const [pipeOpen, setPipeOpen] = useState(false);
  const [pipeRules, setPipeRules] = useState<PipeRule[]>([]);
  const [log, setLog] = useState<LogEntry[]>(loadStoredLog);
  const [signalOpen, setSignalOpen] = useState(false);
  const [signalMessage, setSignalMessage] = useState("");
  const [signalPending, setSignalPending] = useState(false);
  const [signalFlashIds, setSignalFlashIds] = useState<Set<string>>(new Set());
  const [signalFailIds, setSignalFailIds] = useState<Set<string>>(new Set());
  const [signalError, setSignalError] = useState<string | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Monotonic log entry counter — useRef avoids module-level mutable state.
  const logSeqRef = useRef(0);
  // Mirror of sessions for use inside event listener closures without staleness.
  const sessionsRef = useRef<SessionState[]>([]);
  const signalUndoRef = useRef<ReturnType<typeof setTimeout> | null>(null);
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

  useEffect(() => {
    loadSessions();
    loadPipeRules();

    const unlisten = listen<SessionState>("session-updated", (event) => {
      setSessions((prev) => {
        const idx = prev.findIndex((s) => s.id === event.payload.id);
        const next =
          idx >= 0
            ? prev.map((s, i) => (i === idx ? event.payload : s))
            : [...prev, event.payload];
        sessionsRef.current = next;
        return next;
      });
    });

    const interval = setInterval(loadSessions, 5_000);
    return () => {
      unlisten.then((f) => f());
      clearInterval(interval);
    };
  }, [loadSessions, loadPipeRules]);

  // Persist activity log to localStorage on every change.
  useEffect(() => {
    try {
      localStorage.setItem(LOG_KEY, JSON.stringify(log.slice(0, LOG_MAX)));
    } catch {
      // localStorage unavailable in some Tauri configs — silent fail.
    }
  }, [log]);

  // Listen for pipe-fired events — animate and log.
  useEffect(() => {
    let cancelAnim: (() => void) | null = null;

    const unlisten = listen<PipeFiredEvent>("pipe-fired", (event) => {
      const { from_session_id, to_session_id, success, error } = event.payload;

      // Only animate the pipe line on successful injection.
      if (success) {
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
            cancelAnim = animatePipeLine(canvas, fromEl, toEl);
          }
        }
      }

      // Use the ref so this closure is never stale.
      const current = sessionsRef.current;
      const fromName =
        current.find((s) => s.id === from_session_id)?.project ??
        from_session_id.slice(0, 8);
      const toName =
        current.find((s) => s.id === to_session_id)?.project ??
        to_session_id.slice(0, 8);

      setLog((prev) => {
        const text = success
          ? `pipe fired: ${fromName} → ${toName}`
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
  }, []); // stable — reads sessionsRef, not sessions state

  // Resize canvas to window.
  useEffect(() => {
    function resize() {
      const c = canvasRef.current;
      if (!c) return;
      c.width = window.innerWidth;
      c.height = window.innerHeight;
    }
    resize();
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);

  // Reload rules whenever pipe drawer opens or closes.
  useEffect(() => {
    loadPipeRules();
  }, [pipeOpen, loadPipeRules]);

  // Clear any pending signal-undo timeout on unmount so we don't fire
  // invoke() against a torn-down component.
  useEffect(() => {
    return () => {
      if (signalUndoRef.current) {
        clearTimeout(signalUndoRef.current);
        signalUndoRef.current = null;
      }
    };
  }, []);

  // Close pipe drawer and signal bar with Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPipeOpen(false);
        handleSignalCancel();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Listen for signal-fired events — flash cards and log.
  useEffect(() => {
    const unlisten = listen<SignalFiredEvent>("signal-fired", (event) => {
      const { success_ids, fail_ids, message, success_count, fail_count } = event.payload;

      if (success_ids.length > 0) {
        setSignalFlashIds(new Set(success_ids));
        setTimeout(() => setSignalFlashIds(new Set()), 800);
      }
      if (fail_ids.length > 0) {
        setSignalFailIds(new Set(fail_ids));
        setTimeout(() => setSignalFailIds(new Set()), 1000);
      }

      const preview = message.length > 40 ? message.slice(0, 40) + "..." : message;
      const failNote = fail_count > 0 ? ` (${fail_count} failed)` : "";
      setLog((prev) => {
        const entry: LogEntry = {
          id: logSeqRef.current++,
          text: `⌁ signal → ${success_count} sessions: ${preview}${failNote}`,
          ts: now(),
        };
        return [entry, ...prev].slice(0, LOG_MAX);
      });
    });

    return () => { unlisten.then((f) => f()); };
  }, []);

  function handleSignalSubmit() {
    const msg = signalMessage.trim();
    if (!msg || signalPending) return;

    setSignalPending(true);
    setSignalError(null);

    signalUndoRef.current = setTimeout(async () => {
      signalUndoRef.current = null;
      try {
        const results = await invoke<Array<{ session_id: string; project: string; success: boolean; error?: string }>>(
          "signal_sessions",
          { message: msg }
        );
        const allFailed = results.length > 0 && results.every((r) => !r.success);
        if (allFailed) {
          setSignalError("Signal failed — no sessions received it");
          setSignalPending(false);
        } else {
          setSignalMessage("");
          setSignalOpen(false);
          setSignalPending(false);
          setSignalError(null);
        }
      } catch (err) {
        setSignalError(`Signal failed: ${err}`);
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
            {runningCount > 0 ? `${runningCount} running` : "idle"}
          </div>
          <button
            style={{
              ...styles.pipeBtn,
              ...(pipeOpen ? styles.pipeBtnActive : {}),
            }}
            onClick={() => setPipeOpen((v) => !v)}
          >
            Pipes {pipeOpen ? "▼" : "▲"}
            {pipeRules.length > 0 && (
              <span
                style={{
                  marginLeft: "5px",
                  background: "var(--signal)",
                  color: "#000",
                  borderRadius: "8px",
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
            style={{
              ...styles.pipeBtn,
              ...(signalOpen ? styles.pipeBtnActive : {}),
              ...(nonIdleCount === 0 ? { opacity: 0.35, cursor: "not-allowed" } : {}),
            }}
            title={nonIdleCount === 0 ? "No active sessions" : `Broadcast to ${nonIdleCount} session${nonIdleCount !== 1 ? "s" : ""}`}
            onClick={() => {
              if (nonIdleCount === 0) return;
              setSignalOpen((v) => !v);
              setSignalError(null);
            }}
            disabled={nonIdleCount === 0}
          >
            ⌁ Signal
          </button>
        </div>
      </header>

      {signalOpen && (
        <div
          className={`signal-command-bar${signalError ? " signal-command-bar--error" : ""}`}
          style={{ position: "relative" }}
        >
          <input
            ref={signalInputRef}
            className="signal-command-bar__input"
            placeholder="Broadcast to all active sessions..."
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
          {signalMessage.length > 409 && (
            <span className="signal-command-bar__counter">
              {signalMessage.length}/512
            </span>
          )}
          {signalPending && (
            <span className="signal-command-bar__toast">
              Queued for {nonIdleCount} session{nonIdleCount !== 1 ? "s" : ""} —{" "}
              <button
                className="signal-command-bar__cancel"
                onClick={handleSignalCancel}
              >
                Undo
              </button>
            </span>
          )}
          {signalError && (
            <span className="signal-command-bar__error-text">{signalError}</span>
          )}
        </div>
      )}

      <main style={styles.main}>
        {!loading && sessions.length === 0 ? (
          <div style={styles.empty}>
            <div>--</div>
            <div>No Claude sessions found</div>
            <div style={{ color: "#222", fontSize: "11px" }}>
              Sessions appear here when Claude CLI writes to ~/.claude/projects/
            </div>
          </div>
        ) : (
          <div style={styles.grid}>
            {sessions.map((session) => (
              <SessionCard
                key={session.id}
                session={session}
                isSource={sourceIds.has(session.id)}
                isTarget={targetIds.has(session.id)}
                signalSuccess={signalFlashIds.has(session.id)}
                signalFail={signalFailIds.has(session.id)}
              />
            ))}
          </div>
        )}
      </main>

      {/* Activity log */}
      {log.length > 0 && (
        <div className="activity-log">
          {log.slice(0, 5).map((entry, i) => (
            <span
              key={entry.id}
              className="activity-log__entry"
              style={{ opacity: 1 - i * 0.18 }}
            >
              <span style={{ color: "#333", marginRight: "6px" }}>
                {entry.ts}
              </span>
              <span className="activity-log__signal">▸</span> {entry.text}
              {i < Math.min(log.length, 5) - 1 && (
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
            onRulesChanged={loadPipeRules}
            onClose={() => setPipeOpen(false)}
          />
        </>
      )}
    </div>
  );
}
