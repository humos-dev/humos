import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState, SessionStatus } from "./types";
import { formatDateTime } from "./utils/formatDateTime";

interface Props {
  session: SessionState;
  isSource?: boolean;
  isTarget?: boolean;
}

const STATUS_DOT: Record<SessionStatus, { color: string; label: string }> = {
  running: { color: "#3ecf8e", label: "running" },
  waiting: { color: "#eab308", label: "waiting" },
  idle:    { color: "#555",    label: "idle" },
};

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

export function SessionCard({ session, isSource, isTarget }: Props) {
  const [sendOpen, setSendOpen] = useState(false);
  const [message, setMessage] = useState("");
  const [summary, setSummary] = useState<string | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [dots, setDots] = useState(".");
  const [actionError, setActionError] = useState<string | null>(null);

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

  async function handleFocus() {
    try {
      await invoke("focus_session", { sessionId: session.id });
    } catch (err) {
      setActionError("Focus failed — Terminal window not found");
      console.error(err);
    }
  }

  async function handleSend() {
    if (!message.trim()) return;
    try {
      await invoke("inject_message", { sessionId: session.id, message: message.trim() });
      setMessage("");
      setSendOpen(false);
    } catch (err) {
      setActionError(`Send failed: ${err}`);
      console.error(err);
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
  ].filter(Boolean).join(" ");

  return (
    <div className={cardClass} data-session-id={session.id}>
      {/* Header */}
      <div style={styles.cardHeader}>
        <div style={{ flex: 1 }}>
          <div style={styles.projectName}>{session.project || session.id}</div>
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

      {/* Last output */}
      <div style={styles.lastOutput}>
        {session.last_output || <span style={{ color: "#222" }}>no output yet</span>}
      </div>

      {/* Actions — hidden until hover (CSS handles it via session-card__actions) */}
      <div className="session-card__actions">
        <button style={styles.btn} onClick={handleFocus}>Focus</button>
        <button
          style={{ ...styles.btn, ...(sendOpen ? styles.btnPrimary : {}) }}
          onClick={() => setSendOpen((v) => !v)}
        >
          {sendOpen ? "Cancel" : "Send"}
        </button>
        <button
          style={{ ...styles.btn, opacity: summarizing ? 0.5 : 1 }}
          onClick={handleSummarize}
          disabled={summarizing}
        >
          {summarizing ? dots : "Summarize"}
        </button>
      </div>

      {/* Send input */}
      {sendOpen && (
        <input
          style={styles.sendInput}
          placeholder="Type a message and press Enter..."
          value={message}
          autoFocus
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
    </div>
  );
}
