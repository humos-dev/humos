import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SessionCard } from "./SessionCard";
import type { SessionState } from "./types";

const styles: Record<string, React.CSSProperties> = {
  app: {
    minHeight: "100vh",
    background: "#0a0a0a",
    display: "flex",
    flexDirection: "column",
  },
  header: {
    padding: "16px 24px",
    borderBottom: "1px solid #1e1e1e",
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    background: "#0d0d0d",
  },
  title: {
    fontSize: "14px",
    fontWeight: 600,
    color: "#fff",
    letterSpacing: "0.05em",
    textTransform: "uppercase",
  },
  subtitle: {
    fontSize: "11px",
    color: "#555",
    marginTop: "2px",
  },
  badge: {
    background: "#1a1a1a",
    border: "1px solid #2a2a2a",
    borderRadius: "4px",
    padding: "4px 10px",
    fontSize: "11px",
    color: "#777",
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
  emptyIcon: {
    fontSize: "32px",
    marginBottom: "8px",
  },
};

export default function App() {
  const [sessions, setSessions] = useState<SessionState[]>([]);
  const [loading, setLoading] = useState(true);

  const loadSessions = useCallback(async () => {
    try {
      const result = await invoke<SessionState[]>("get_sessions");
      setSessions(result);
    } catch (err) {
      console.error("Failed to load sessions:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadSessions();

    // Listen for real-time session updates from the file watcher
    const unlisten = listen<SessionState>("session-updated", (event) => {
      setSessions((prev) => {
        const idx = prev.findIndex((s) => s.id === event.payload.id);
        if (idx >= 0) {
          const next = [...prev];
          next[idx] = event.payload;
          return next;
        }
        return [...prev, event.payload];
      });
    });

    // Poll every 30s as a fallback
    const interval = setInterval(loadSessions, 30_000);

    return () => {
      unlisten.then((f) => f());
      clearInterval(interval);
    };
  }, [loadSessions]);

  const runningCount = sessions.filter((s) => s.status === "running").length;

  return (
    <div style={styles.app}>
      <header style={styles.header}>
        <div>
          <div style={styles.title}>HumOS</div>
          <div style={styles.subtitle}>
            {loading ? "Scanning..." : `${sessions.length} session${sessions.length !== 1 ? "s" : ""} detected`}
          </div>
        </div>
        <div style={styles.badge}>
          {runningCount > 0 ? `${runningCount} running` : "no active sessions"}
        </div>
      </header>

      <main style={styles.main}>
        {!loading && sessions.length === 0 ? (
          <div style={styles.empty}>
            <div style={styles.emptyIcon}>--</div>
            <div>No Claude sessions found</div>
            <div style={{ color: "#222", fontSize: "11px" }}>
              Sessions appear here when Claude CLI writes to ~/.claude/projects/
            </div>
          </div>
        ) : (
          <div style={styles.grid}>
            {sessions.map((session) => (
                <SessionCard key={session.id} session={session} />
              ))}
          </div>
        )}
      </main>
    </div>
  );
}
