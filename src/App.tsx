import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SessionCard } from "./SessionCard";
import { PipeConfig } from "./PipeConfig";
import type { SessionState } from "./types";

interface PipeFiredEvent {
  rule_id: string;
  from_session_id: string;
  to_session_id: string;
  message: string;
}

interface PipeRule {
  id: string;
  from_session_id: string;
  to_session_id: string;
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
      setTimeout(() => {
        toEl.style.transition = "box-shadow 0.6s ease";
        toEl.style.boxShadow = "";
        setTimeout(() => {
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
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Monotonic log entry counter — useRef avoids module-level mutable state.
  const logSeqRef = useRef(0);
  // Mirror of sessions for use inside event listener closures without staleness.
  const sessionsRef = useRef<SessionState[]>([]);

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
      const { from_session_id, to_session_id } = event.payload;

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

      // Use the ref so this closure is never stale.
      const current = sessionsRef.current;
      const fromName =
        current.find((s) => s.id === from_session_id)?.project ??
        from_session_id.slice(0, 8);
      const toName =
        current.find((s) => s.id === to_session_id)?.project ??
        to_session_id.slice(0, 8);

      setLog((prev) => {
        const entry: LogEntry = {
          id: logSeqRef.current++,
          text: `pipe fired: ${fromName} → ${toName}`,
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

  // Reload rules when pipe drawer closes so cards update their indicators.
  useEffect(() => {
    if (!pipeOpen) loadPipeRules();
  }, [pipeOpen, loadPipeRules]);

  // Close pipe drawer with Escape.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPipeOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const runningCount = sessions.filter((s) => s.status === "running").length;
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
        </div>
      </header>

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
            onClick={() => setPipeOpen(false)}
          />
          <PipeConfig sessions={sessions} />
        </>
      )}
    </div>
  );
}
