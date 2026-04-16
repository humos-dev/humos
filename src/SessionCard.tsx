import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState, SessionStatus } from "./types";
import { formatDateTime } from "./utils/formatDateTime";

interface RibbonEntry {
  session_id: string;
  project: string;
  cwd: string;
  snippet: string;
  modified_at: string;
}

interface RibbonResult {
  daemon_online: boolean;
  is_stale: boolean;
  entries: RibbonEntry[];
  total_count: number;
}

type RibbonState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "results"; data: RibbonResult }
  | { kind: "error" };

interface Props {
  session: SessionState;
  isSource?: boolean;
  isTarget?: boolean;
  signalSuccess?: boolean;
  signalFail?: boolean;
  daemonOnline?: boolean;
}

const STATUS_DOT: Record<SessionStatus, { color: string; label: string }> = {
  running: { color: "#3ecf8e", label: "running" },
  waiting: { color: "#eab308", label: "waiting" },
  idle:    { color: "#555",    label: "idle" },
};

// Per-provider badge appearance. Unknown providers fall back to "?" in a
// muted tone so new integrations never crash the card.
const PROVIDER_BADGE: Record<string, { label: string; color: string; border: string; bg: string }> = {
  claude:  { label: "Claude", color: "#3ecf8e", border: "rgba(62, 207, 142, 0.35)", bg: "rgba(62, 207, 142, 0.08)" },
  codex:   { label: "Codex",  color: "#a78bfa", border: "rgba(167, 139, 250, 0.35)", bg: "rgba(167, 139, 250, 0.08)" },
};
const PROVIDER_FALLBACK = { label: "?", color: "#777", border: "#333", bg: "#1a1a1a" };

const styles: Record<string, React.CSSProperties> = {
  cardHeader: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "8px",
  },
  projectName: {
    fontWeight: 700,
    fontSize: "13px",
    color: "var(--text)",
    letterSpacing: "0.02em",
    lineHeight: 1.3,
    wordBreak: "break-all",
  },
  cwd: {
    fontSize: "10px",
    color: "#666",
    marginTop: "3px",
    wordBreak: "break-all",
  },
  statusRow: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  dot: {
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  statusLabel: {
    fontSize: "11px",
    color: "var(--text-2)",
    textTransform: "uppercase",
    letterSpacing: "0.08em",
  },
  toolBadge: {
    marginLeft: "auto",
    background: "#1a1a1a",
    border: "1px solid #333",
    borderRadius: "4px",
    padding: "2px 8px",
    fontSize: "10px",
    color: "var(--text-2)",
  },
  lastOutput: {
    fontSize: "11px",
    color: "#666",
    lineHeight: 1.5,
    background: "#0d0d0d",
    border: "1px solid #1a1a1a",
    borderRadius: "4px",
    padding: "8px 10px",
    minHeight: "36px",
    maxHeight: "60px",
    overflowY: "hidden" as const,
    wordBreak: "break-word",
    whiteSpace: "pre-wrap",
  },
  btn: {
    flex: 1,
    padding: "6px 12px",
    fontSize: "11px",
    fontFamily: "inherit",
    borderRadius: "4px",
    border: "1px solid #2a2a2a",
    background: "#1a1a1a",
    color: "#aaa",
    cursor: "pointer",
    transition: "background 0.1s, color 0.1s",
    letterSpacing: "0.04em",
  },
  btnPrimary: {
    background: "#1a2a1a",
    border: "1px solid #2a4a2a",
    color: "#4ade80",
  },
  sendInput: {
    width: "100%",
    padding: "8px 10px",
    fontSize: "11px",
    fontFamily: "inherit",
    background: "#0d0d0d",
    border: "1px solid #2a4a2a",
    borderRadius: "4px",
    color: "var(--text)",
    outline: "none",
    marginTop: "-4px",
  },
  summaryOverlay: {
    position: "absolute",
    inset: 0,
    background: "#0d0d0dee",
    backdropFilter: "blur(2px)",
    borderRadius: "8px",
    padding: "16px",
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    zIndex: 10,
    overflow: "hidden",
  },
  summaryTitle: {
    fontSize: "10px",
    color: "var(--text-2)",
    textTransform: "uppercase",
    letterSpacing: "0.08em",
  },
  summaryText: {
    fontSize: "12px",
    color: "#ccc",
    lineHeight: 1.6,
    flex: 1,
    overflowY: "auto",
  },
  summaryClose: {
    alignSelf: "flex-end",
    background: "none",
    border: "1px solid #2a2a2a",
    borderRadius: "4px",
    color: "var(--text-2)",
    fontSize: "10px",
    padding: "4px 10px",
    cursor: "pointer",
    fontFamily: "inherit",
    letterSpacing: "0.04em",
  },
  timestamp: {
    fontSize: "10px",
    color: "#555",
    lineHeight: 1.5,
  },
};

export function SessionCard({ session, isSource, isTarget, signalSuccess, signalFail, daemonOnline }: Props) {
  const [sendOpen, setSendOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [message, setMessage] = useState("");
  const [sending, setSending] = useState(false);
  const [focused, setFocused] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [dots, setDots] = useState(".");
  const [actionError, setActionError] = useState<string | null>(null);
  const [ribbon, setRibbon] = useState<RibbonState>({ kind: "idle" });
  const [cardFocused, setCardFocused] = useState(false);
  const ribbonDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!summarizing) return;
    const interval = setInterval(() => {
      setDots((d) => (d.length >= 3 ? "." : d + "."));
    }, 400);
    return () => clearInterval(interval);
  }, [summarizing]);

  useEffect(() => {
    if (actionError) {
      const t = setTimeout(() => setActionError(null), 3000);
      return () => clearTimeout(t);
    }
  }, [actionError]);

  // Project Brain ribbon: fetch on card focus, 200ms debounce.
  useEffect(() => {
    if (ribbonDebounceRef.current) clearTimeout(ribbonDebounceRef.current);
    if (!cardFocused || !daemonOnline || !session.cwd) {
      setRibbon({ kind: "idle" });
      return;
    }
    ribbonDebounceRef.current = setTimeout(async () => {
      setRibbon({ kind: "loading" });
      try {
        const result = await invoke<RibbonResult>("get_related_context", { cwd: session.cwd });
        setRibbon({ kind: "results", data: result });
      } catch {
        setRibbon({ kind: "error" });
      }
    }, 200);
    return () => {
      if (ribbonDebounceRef.current) clearTimeout(ribbonDebounceRef.current);
    };
  }, [cardFocused, daemonOnline, session.cwd]);

  const statusInfo = STATUS_DOT[session.status];
  const isRunning = session.status === "running";

  async function handleFocus() {
    try {
      await invoke("focus_session", { sessionId: session.id, cwd: session.cwd });
      setFocused(true);
      setTimeout(() => setFocused(false), 1500);
    } catch (err) {
      setActionError(`${err}`);
      setTimeout(() => setActionError(null), 5000);
      setFocused(false);
      console.error(err);
    }
  }

  async function handleSend() {
    if (!message.trim() || sending) return;
    setSending(true);
    setActionError(null);
    try {
      await invoke("inject_message", {
        sessionId: session.id,
        message: message.trim(),
        cwd: session.cwd,
      });
      setMessage("");
      setSendOpen(false);
    } catch (err) {
      setActionError(`${err}`);
      setTimeout(() => setActionError(null), 5000);
      console.error(err);
    } finally {
      setSending(false);
    }
  }

  async function handleSummarize() {
    setSummarizing(true);
    try {
      const raw = await invoke<string>("summarize_session", { sessionId: session.id });
      setSummary(raw || "(no content)");
    } catch (err) {
      setSummary(`Error: ${err}`);
    } finally {
      setSummarizing(false);
    }
  }

  const cardClass = [
    "session-card",
    isRunning ? "session-card--running" : "",
    sendOpen ? "session-card--send-open" : "",
    signalSuccess ? "session-card--signal-success" : "",
    signalFail ? "session-card--signal-fail" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className={cardClass}
      data-session-id={session.id}
      onMouseEnter={() => setCardFocused(true)}
      onMouseLeave={() => { setCardFocused(false); setRibbon({ kind: "idle" }); }}
    >
      {/* Header */}
      <div style={styles.cardHeader}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
            <div style={styles.projectName}>{session.project || session.id}</div>
            {(() => {
              const badge = PROVIDER_BADGE[session.provider] ?? PROVIDER_FALLBACK;
              return (
                <span
                  title={`Provider: ${session.provider || "unknown"}`}
                  style={{
                    fontSize: "9px",
                    fontWeight: 600,
                    letterSpacing: "0.06em",
                    textTransform: "uppercase",
                    color: badge.color,
                    background: badge.bg,
                    border: `1px solid ${badge.border}`,
                    borderRadius: "3px",
                    padding: "1px 5px",
                    lineHeight: 1.4,
                    flexShrink: 0,
                  }}
                >
                  {badge.label}
                </span>
              );
            })()}
          </div>
          <div style={styles.cwd}>{session.cwd}</div>
        </div>
        {/* Pipe connection dot — no label, just presence */}
        {(isSource || isTarget) && (
          <div
            title={isSource ? "pipe source" : "pipe target"}
            style={{
              width: "5px",
              height: "5px",
              borderRadius: "50%",
              background: "var(--signal)",
              opacity: 0.5,
              flexShrink: 0,
              marginTop: "3px",
            }}
          />
        )}
      </div>

      {/* Status row */}
      <div style={styles.statusRow}>
        <span
          className={isRunning ? "session-card__dot--running" : ""}
          style={{ ...styles.dot, background: statusInfo.color }}
        />
        <span style={styles.statusLabel}>{statusInfo.label}</span>
        <span style={styles.toolBadge}>
          {session.tool_count} tool{session.tool_count !== 1 ? "s" : ""}
        </span>
        <span style={{ ...styles.timestamp, marginLeft: "auto", textAlign: "right" }}>
          {(() => {
            const { date, time } = formatDateTime(session.modified_at);
            return date ? <>{date}<br />{time}</> : null;
          })()}
        </span>
      </div>

      {/* Activity (click to expand) */}
      <div
        className={`session-card__activity${expanded ? " session-card__activity--expanded" : ""}`}
        onClick={() => setExpanded((v) => !v)}
      >
        <div className="session-card__activity-text">
          {(session.last_output || "no output yet").replace(/\*\*/g, "").replace(/`/g, "")}
        </div>
        {expanded && (
          <>
            {session.recent_tools && session.recent_tools.length > 0 && (
              <div className="session-card__activity-tools">
                {session.recent_tools.map((t, i) => (
                  <span key={i} className="session-card__tool-tag">{t}</span>
                ))}
              </div>
            )}
            <div className="session-card__activity-meta">
              <span>{formatDateTime(session.started_at).date} {formatDateTime(session.started_at).time}</span>
              <span className="session-card__meta-sep">·</span>
              <span>{session.tool_count} tool{session.tool_count !== 1 ? "s" : ""}</span>
              <span className="session-card__meta-sep">·</span>
              <span>{session.status}</span>
            </div>
          </>
        )}
        <span className="session-card__expand-icon">{expanded ? "▾" : "▸"}</span>
      </div>

      {/* Actions — hidden until hover (CSS handles it via session-card__actions) */}
      <div className="session-card__actions">
        <button
          style={{ ...styles.btn, ...(focused ? styles.btnPrimary : {}) }}
          onClick={handleFocus}
          aria-label={`Focus ${session.project} session`}
        >
          {focused ? "Focused!" : "Focus"}
        </button>
        <button
          style={{ ...styles.btn, ...(sendOpen ? styles.btnPrimary : {}) }}
          onClick={() => setSendOpen((v) => !v)}
          aria-label={`Send message to ${session.project}`}
        >
          {sendOpen ? "Cancel" : "Send"}
        </button>
        <button
          style={{ ...styles.btn, opacity: summarizing ? 0.5 : 1 }}
          onClick={handleSummarize}
          disabled={summarizing}
          aria-label={`Summarize ${session.project} session`}
        >
          {summarizing ? dots : "Summarize"}
        </button>
      </div>

      {/* Send input */}
      {sendOpen && (
        <input
          style={styles.sendInput}
          placeholder={`Send to ${session.project}... (Enter to send, Esc to cancel)`}
          value={message}
          maxLength={512}
          autoFocus
          readOnly={sending}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
            if (e.key === "Escape") setSendOpen(false);
          }}
        />
      )}

      {/* Summary overlay */}
      {summary !== null && !summarizing && (
        <div style={styles.summaryOverlay}>
          <div style={styles.summaryTitle}>Summary</div>
          <div style={styles.summaryText}>{summary}</div>
          <button style={styles.summaryClose} onClick={() => setSummary(null)}>Dismiss</button>
        </div>
      )}

      {/* Action error */}
      {actionError && (
        <div style={{ fontSize: "11px", color: "#f87171", marginTop: "-4px" }}>
          {actionError}
        </div>
      )}

      {/* Project Brain ribbon — shows on hover when daemon is online */}
      {cardFocused && daemonOnline && (
        <div style={{
          marginTop: "10px",
          borderTop: "1px solid #1e1e1e",
          paddingTop: "8px",
        }}>
          {ribbon.kind === "loading" && (
            <div style={{ fontSize: "10px", color: "#444" }}>Loading past sessions...</div>
          )}
          {ribbon.kind === "results" && ribbon.data.entries.length === 0 && (
            <div style={{ fontSize: "10px", color: "#444" }}>
              No past sessions in this repo.
            </div>
          )}
          {ribbon.kind === "results" && ribbon.data.entries.length > 0 && (
            <div>
              <div style={{ fontSize: "9px", color: "#555", textTransform: "uppercase", letterSpacing: "0.08em", marginBottom: "5px" }}>
                Project Brain
                {ribbon.data.is_stale && (
                  <span style={{ marginLeft: "6px", color: "#5a5a2a" }}>· Updating index...</span>
                )}
              </div>
              {ribbon.data.entries.slice(0, 5).map((entry) => (
                <div key={entry.session_id} style={{
                  marginBottom: "5px",
                  padding: "4px 6px",
                  background: "#0e0e0e",
                  borderRadius: "4px",
                  borderLeft: "2px solid #2a2a2a",
                }}>
                  <div style={{ fontSize: "10px", color: "#888", marginBottom: "2px" }}>
                    {entry.project} · {new Date(entry.modified_at).toLocaleDateString()}
                  </div>
                  <div style={{ fontSize: "11px", color: "#aaa", lineHeight: 1.4 }}>
                    {entry.snippet}
                  </div>
                </div>
              ))}
              {ribbon.data.total_count > 5 && (
                <div style={{ fontSize: "10px", color: "#555" }}>
                  and {ribbon.data.total_count - 5} more
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
