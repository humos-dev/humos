import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState, SessionStatus } from "./types";
import { formatDateTime } from "./utils/formatDateTime";

interface Props {
  session: SessionState;
  isSource?: boolean;
  isTarget?: boolean;
  signalSuccess?: boolean;
  signalFail?: boolean;
  ribbon?: React.ReactNode;
  pipeHistory?: { fromProject: string; ts: number };
  viewMode?: "grid" | "list";
}

function relativeTime(ts: number): string {
  const diff = Math.floor((Date.now() - ts) / 1000);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
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
    borderRadius: "5px",
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

export function SessionCard({ session, isSource, isTarget, signalSuccess, signalFail, ribbon, pipeHistory, viewMode = "grid" }: Props) {
  const [sendOpen, setSendOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [message, setMessage] = useState("");
  const [sending, setSending] = useState(false);
  const [focused, setFocused] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [dots, setDots] = useState(".");
  const [actionError, setActionError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const isIdle = session.status === "idle";

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

  const statusInfo = STATUS_DOT[session.status];
  const isRunning = session.status === "running";
  const hasRibbon = !!ribbon;

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
    isIdle ? "session-card--idle" : "",
    sendOpen ? "session-card--send-open" : "",
    signalSuccess ? "session-card--signal-success" : "",
    signalFail ? "session-card--signal-fail" : "",
    (isSource || isTarget) ? "session-card--coord" : "",
  ].filter(Boolean).join(" ");

  // List view render
  if (viewMode === "list") {
    const pipeCell = pipeHistory
      ? (isSource
        ? `→ ${pipeHistory.fromProject} · ${relativeTime(pipeHistory.ts)}`
        : `← ${pipeHistory.fromProject} · ${relativeTime(pipeHistory.ts)}`)
      : (isSource || isTarget ? "connected" : "—");
    return (
      <>
        <div
          className={[
            "session-list__row",
            session.status === "running" || session.status === "waiting" ? `session-list__row--${session.status}` : "session-list__row--idle",
            (isSource || isTarget) ? "session-list__row--coord" : "",
          ].filter(Boolean).join(" ")}
          data-session-id={session.id}
        >
          <div className="session-list__cell session-list__cell--name">
            <div style={{ fontWeight: 600, fontSize: "11px", color: "var(--text)" }}>{session.project || session.id}</div>
            <div style={{ fontSize: "9px", color: "var(--text-3)", marginTop: "1px" }}>{session.cwd}</div>
          </div>
          <div className="session-list__cell session-list__cell--status">
            <div style={{ fontSize: "9px", textTransform: "uppercase", letterSpacing: "0.08em", color: statusInfo.color }}>{statusInfo.label}</div>
            <div style={{ fontSize: "8px", color: "#444", marginTop: "1px" }}>{session.tool_count} tools</div>
          </div>
          <div className="session-list__cell session-list__cell--output">
            <div style={{ fontSize: "10px", color: "var(--text-3)", lineHeight: 1.4 }}>
              {(session.last_output || "no output").replace(/\*\*/g, "").replace(/`/g, "").slice(0, 80)}
            </div>
          </div>
          <div className="session-list__cell session-list__cell--pipe">
            <div style={{ fontSize: "9px", color: pipeHistory ? "var(--coord)" : "#444", opacity: pipeHistory ? 0.8 : 1 }}>{pipeCell}</div>
          </div>
          <div className="session-list__cell session-list__cell--ts">
            <div style={{ fontSize: "9px", color: "#444" }}>{(() => { const { time } = formatDateTime(session.modified_at); return time; })()}</div>
            {isIdle && (
              <button
                style={{ marginTop: "3px", background: "rgba(248,113,113,.07)", border: "1px solid rgba(248,113,113,.2)", color: "var(--error)", borderRadius: "2px", padding: "1px 6px", fontFamily: "inherit", fontSize: "8px", cursor: "pointer" }}
                onClick={() => setSendOpen((v) => !v)}
              >Ended</button>
            )}
          </div>
          <div className="session-list__cell session-list__cell--actions">
            <button
              style={{ ...styles.btn, padding: "3px 8px", fontSize: "9px", ...(focused ? styles.btnPrimary : {}) }}
              onClick={handleFocus}
              aria-label={`Focus ${session.project}`}
            >{focused ? "Focused!" : "Focus"}</button>
            <button
              style={{ ...styles.btn, padding: "3px 8px", fontSize: "9px", marginTop: "3px", opacity: summarizing ? 0.5 : 1 }}
              onClick={handleSummarize}
              disabled={summarizing}
              aria-label={`Summarize ${session.project}`}
            >{summarizing ? dots : "Summarize"}</button>
          </div>
        </div>
        {isIdle && sendOpen && (
          <div className="session-list__dead-row">
            <span>Session ended. Resume:</span>
            <code style={{ color: "var(--error)", fontSize: "9px", margin: "0 6px" }}>claude --resume {session.id}</code>
            <button
              style={{ marginLeft: "auto", background: "rgba(248,113,113,.07)", border: "1px solid rgba(248,113,113,.2)", color: "var(--error)", borderRadius: "2px", padding: "2px 8px", fontFamily: "inherit", fontSize: "9px", cursor: "pointer" }}
              onClick={() => { navigator.clipboard.writeText(`claude --resume ${session.id}`).catch(() => {}); setCopied(true); setTimeout(() => setCopied(false), 1500); }}
            >{copied ? "Copied!" : "Copy"}</button>
          </div>
        )}
        {summary !== null && !summarizing && (
          <div style={{ display: "flex", alignItems: "flex-start", gap: "10px", padding: "8px 12px", background: "#0d0d0d", borderBottom: "1px solid var(--border)" }}>
            <div style={{ flex: 1, fontSize: "11px", color: "#ccc", lineHeight: 1.6 }}>{summary}</div>
            <button style={{ ...styles.btn, padding: "3px 10px", fontSize: "9px", flexShrink: 0 }} onClick={() => setSummary(null)}>Dismiss</button>
          </div>
        )}
      </>
    );
  }

  return (
    <div className={cardClass} data-session-id={session.id}>
      {/* Project Brain ribbon — ambient strip at card top (spec v2) */}
      {ribbon}

      {/* Header */}
      <div style={styles.cardHeader}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
            <div style={styles.projectName}>{session.project || session.id}</div>
            {(() => {
              const badge = PROVIDER_BADGE[session.provider] ?? PROVIDER_FALLBACK;
              // Demote to neutral when ribbon present (no double-green anchor)
              const badgeColor = hasRibbon ? "#777" : badge.color;
              const badgeBg = hasRibbon ? "#151515" : badge.bg;
              const badgeBorder = hasRibbon ? "#262626" : badge.border;
              return (
                <span
                  title={`Provider: ${session.provider || "unknown"}`}
                  style={{
                    fontSize: "9px",
                    fontWeight: 600,
                    letterSpacing: "0.06em",
                    textTransform: "uppercase",
                    color: badgeColor,
                    background: badgeBg,
                    border: `1px solid ${badgeBorder}`,
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
        {/* Pipe connection dot */}
        {(isSource || isTarget) && (
          <div
            title={isSource ? "pipe source" : "pipe target"}
            style={{
              width: "5px",
              height: "5px",
              borderRadius: "50%",
              background: "var(--coord)",
              opacity: 0.7,
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
        <span className="session-card__expand-icon">{expanded ? "\u25BE" : "\u25B8"}</span>
      </div>

      {/* Actions — hidden until hover (CSS handles it) */}
      <div className="session-card__actions">
        <button
          style={{ ...styles.btn, ...(focused ? styles.btnPrimary : {}) }}
          onClick={handleFocus}
          aria-label={`Focus ${session.project} session`}
        >
          {focused ? "Focused!" : "Focus"}
        </button>
        <button
          style={{
            ...styles.btn,
            ...(sendOpen ? styles.btnPrimary : {}),
            ...(isIdle ? { opacity: 0.5 } : {}),
          }}
          onClick={() => setSendOpen((v) => !v)}
          aria-label={isIdle ? `Session ended — see resume command` : `Send message to ${session.project}`}
        >
          {isIdle ? "Ended" : sendOpen ? "Cancel" : "Send"}
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

      {/* Send input / Dead session callout */}
      {sendOpen && (isIdle ? (
        <div style={{
          padding: "7px 10px",
          background: "rgba(248, 113, 113, 0.06)",
          border: "1px solid rgba(248, 113, 113, 0.2)",
          borderRadius: "3px",
          fontSize: "10px",
          color: "var(--text-2)",
          lineHeight: 1.6,
          display: "flex",
          alignItems: "center",
          gap: "8px",
        }}>
          <span>Session ended. Resume:</span>
          <code style={{ color: "var(--error)", fontSize: "9px" }}>
            claude --resume {session.id}
          </code>
          <button
            style={{
              marginLeft: "auto",
              background: "rgba(248,113,113,.07)",
              border: "1px solid rgba(248,113,113,.2)",
              color: "var(--error)",
              borderRadius: "2px",
              padding: "2px 8px",
              fontFamily: "inherit",
              fontSize: "9px",
              cursor: "pointer",
            }}
            onClick={() => {
              navigator.clipboard.writeText(`claude --resume ${session.id}`).catch(() => {});
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
          >{copied ? "Copied!" : "Copy"}</button>
        </div>
      ) : (
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
      ))}

      {/* Summary overlay */}
      {summary !== null && !summarizing && (
        <div style={styles.summaryOverlay}>
          <div style={styles.summaryTitle}>Summary</div>
          <div style={styles.summaryText}>{summary}</div>
          <button style={styles.summaryClose} onClick={() => setSummary(null)}>Dismiss</button>
        </div>
      )}

      {/* Pipe history footer */}
      {pipeHistory && (
        <div style={{
          fontSize: "9px",
          color: "#444",
          borderTop: "1px solid var(--border)",
          paddingTop: "5px",
          marginTop: "-2px",
        }}>
          {isSource
            ? <>→ <span style={{ color: "var(--coord)", opacity: 0.7 }}>{pipeHistory.fromProject}</span> · {relativeTime(pipeHistory.ts)}</>
            : <>← <span style={{ color: "var(--coord)", opacity: 0.7 }}>{pipeHistory.fromProject}</span> · {relativeTime(pipeHistory.ts)}</>
          }
        </div>
      )}

      {/* Action error */}
      {actionError && (
        <div style={{ fontSize: "10px", color: "#f87171", marginTop: "-4px" }}>
          {actionError}
        </div>
      )}
    </div>
  );
}
