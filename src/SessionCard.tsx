import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState, SessionStatus } from "./types";

interface Props {
  session: SessionState;
}

const STATUS_DOT: Record<SessionStatus, { color: string; label: string }> = {
  running: { color: "#22c55e", label: "running" },
  waiting: { color: "#eab308", label: "waiting" },
  idle:    { color: "#444",    label: "idle" },
};

const styles: Record<string, React.CSSProperties> = {
  card: {
    background: "#111",
    border: "1px solid #1e1e1e",
    borderRadius: "8px",
    padding: "16px",
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    transition: "border-color 0.15s",
  },
  cardHeader: {
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "space-between",
    gap: "8px",
  },
  projectName: {
    fontWeight: 700,
    fontSize: "13px",
    color: "#fff",
    letterSpacing: "0.02em",
    lineHeight: 1.3,
    wordBreak: "break-all",
  },
  cwd: {
    fontSize: "10px",
    color: "#444",
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
    color: "#555",
    textTransform: "uppercase",
    letterSpacing: "0.08em",
  },
  toolBadge: {
    marginLeft: "auto",
    background: "#1a1a1a",
    border: "1px solid #2a2a2a",
    borderRadius: "4px",
    padding: "2px 8px",
    fontSize: "10px",
    color: "#555",
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
  actions: {
    display: "flex",
    gap: "8px",
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
    color: "#e0e0e0",
    outline: "none",
    marginTop: "-4px",
  },
  summaryBox: {
    fontSize: "10px",
    color: "#555",
    lineHeight: 1.6,
    background: "#0a0a0a",
    border: "1px solid #1a1a1a",
    borderRadius: "4px",
    padding: "8px 10px",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word",
    maxHeight: "120px",
    overflowY: "auto",
  },
  timestamp: {
    fontSize: "10px",
    color: "#333",
  },
};

export function SessionCard({ session }: Props) {
  const [sendOpen, setSendOpen] = useState(false);
  const [message, setMessage] = useState("");
  const [summary, setSummary] = useState<string | null>(null);
  const [summarizing, setSummarizing] = useState(false);

  const statusInfo = STATUS_DOT[session.status];

  async function handleFocus() {
    try {
      await invoke("focus_session", { sessionId: session.id });
    } catch (err) {
      console.error("focus_session error:", err);
    }
  }

  async function handleSend() {
    if (!message.trim()) return;
    try {
      await invoke("inject_message", {
        sessionId: session.id,
        message: message.trim(),
      });
      setMessage("");
      setSendOpen(false);
    } catch (err) {
      console.error("inject_message error:", err);
    }
  }

  async function handleSummarize() {
    setSummarizing(true);
    try {
      const raw = await invoke<string>("summarize_session", {
        sessionId: session.id,
      });
      setSummary(raw || "(no content)");
    } catch (err) {
      setSummary("Error loading session data.");
      console.error(err);
    } finally {
      setSummarizing(false);
    }
  }

  return (
    <div
      style={{
        ...styles.card,
        borderColor: session.status === "running" ? "#1a3a1a" : "#1e1e1e",
      }}
    >
      {/* Header */}
      <div style={styles.cardHeader}>
        <div>
          <div style={styles.projectName}>{session.project || session.id}</div>
          <div style={styles.cwd}>{session.cwd}</div>
        </div>
      </div>

      {/* Status row */}
      <div style={styles.statusRow}>
        <span
          style={{
            ...styles.dot,
            background: statusInfo.color,
            boxShadow: session.status === "running" ? `0 0 6px ${statusInfo.color}` : "none",
          }}
        />
        <span style={styles.statusLabel}>{statusInfo.label}</span>
        <span style={styles.toolBadge}>{session.tool_count} tool{session.tool_count !== 1 ? "s" : ""}</span>
        <span style={{ ...styles.timestamp, marginLeft: "auto" }}>
          {session.modified_at ? new Date(session.modified_at).toLocaleTimeString() : ""}
        </span>
      </div>

      {/* Last output */}
      <div style={styles.lastOutput}>
        {session.last_output || <span style={{ color: "#2a2a2a" }}>no output yet</span>}
      </div>

      {/* Actions */}
      <div style={styles.actions}>
        <button style={styles.btn} onClick={handleFocus}>
          Focus
        </button>
        <button
          style={{ ...styles.btn, ...(sendOpen ? styles.btnPrimary : {}) }}
          onClick={() => setSendOpen((v) => !v)}
        >
          {sendOpen ? "Cancel" : "Send"}
        </button>
        <button
          style={styles.btn}
          onClick={handleSummarize}
          disabled={summarizing}
        >
          {summarizing ? "..." : "Summarize"}
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
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleSend();
            }
            if (e.key === "Escape") setSendOpen(false);
          }}
        />
      )}

      {/* Summary box */}
      {summary !== null && (
        <div style={styles.summaryBox}>{summary}</div>
      )}
    </div>
  );
}
