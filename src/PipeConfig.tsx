import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState, PipeTokenState } from "./types";

type TriggerKind = "on_idle" | "on_file_write";

interface PipeRule {
  id: string;
  from_session_id: string;
  to_session_id: string;
  from_cwd: string;
  to_cwd: string;
  trigger: { OnIdle: null } | { OnFileWrite: string };
}

interface Props {
  sessions: SessionState[];
  rules: PipeRule[];
  tokens?: Map<string, PipeTokenState>;
  onRulesChanged: () => Promise<void>;
  onClose?: () => void;
}

function triggerLabel(rule: PipeRule): string {
  if (typeof rule.trigger === "string") {
    return rule.trigger === "OnIdle" ? "on idle" : `on file write`;
  }
  if ("OnIdle" in rule.trigger) return "on idle";
  if ("OnFileWrite" in rule.trigger) return `on file write (${(rule.trigger as { OnFileWrite: string }).OnFileWrite})`;
  return "unknown";
}

// Format the per-rule token badge. Returns null when neither payload nor source
// has data, which signals "render n/a fallback only" to the caller.
function formatTokenBadge(state: PipeTokenState | undefined, toName: string): React.ReactNode {
  // No fire yet for this rule. Match the wireframe "no pipe fires yet" copy.
  if (!state) {
    return (
      <span style={{ color: "var(--text-3)" }}>No pipe fires yet</span>
    );
  }

  const { payload_tokens, source_tokens, success } = state;
  const failedSuffix = success ? "" : " (failed)";

  // Source has zero token data (opencode sessions report 0 for input/output).
  // Skip the savings line entirely per spec.
  if (source_tokens === 0) {
    return (
      <span style={{ color: "var(--text-3)" }}>tokens: n/a{failedSuffix}</span>
    );
  }

  // Edge: payload >= source means the pipe forwarded the full context (or
  // more, e.g. heavy multibyte content vs tiny session). The "X% lighter"
  // framing is wrong here, so render an alternate copy.
  if (payload_tokens >= source_tokens) {
    return (
      <>
        <div>
          Last fire: {payload_tokens.toLocaleString()} tokens → {toName}{failedSuffix}
        </div>
        <div>
          Source: {source_tokens.toLocaleString()} tokens · forwarded full context
        </div>
      </>
    );
  }

  const savingsPct = Math.round(
    ((source_tokens - payload_tokens) / source_tokens) * 100,
  );

  return (
    <>
      <div>
        Last fire: {payload_tokens.toLocaleString()} tokens → {toName}{failedSuffix}
      </div>
      <div>
        Source: {source_tokens.toLocaleString()} tokens ·{" "}
        <span style={{ color: "var(--coord)" }}>{savingsPct}%</span> lighter
      </div>
    </>
  );
}

export function PipeConfig({ sessions, rules, tokens, onRulesChanged, onClose }: Props) {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [trigger, setTrigger] = useState<TriggerKind>("on_idle");
  const [filePattern, setFilePattern] = useState("*.json");
  const [error, setError] = useState<string | null>(null);

  async function handleAdd() {
    setError(null);
    if (!fromId) { setError("Select a source session (A)."); return; }
    if (!toId) { setError("Select a target session (B)."); return; }
    if (fromId === toId) { setError("Source and target must be different sessions."); return; }
    if (trigger === "on_file_write" && !filePattern.trim()) {
      setError("Enter a file glob pattern (e.g. *.json).");
      return;
    }
    try {
      const fromSession = sessions.find((s) => s.id === fromId);
      const toSession = sessions.find((s) => s.id === toId);
      await invoke("add_pipe_rule", {
        fromSessionId: fromId,
        toSessionId: toId,
        fromCwd: fromSession?.cwd ?? "",
        toCwd: toSession?.cwd ?? "",
        trigger,
        filePattern: trigger === "on_file_write" ? filePattern.trim() : null,
      });
      setFromId("");
      setToId("");
      setFilePattern("*.json");
      await onRulesChanged();
    } catch (err) {
      setError(`Error: ${err}`);
    }
  }

  async function handleRemove(id: string) {
    try {
      await invoke("remove_pipe_rule", { ruleId: id });
      await onRulesChanged();
    } catch (err) {
      console.error("remove_pipe_rule error:", err);
    }
  }

  function sessionLabel(s: SessionState) {
    return `${s.project} (${s.status})`;
  }

  return (
    <div className="pipe-config__drawer" onMouseDown={(e) => e.stopPropagation()}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div className="pipe-config__title">Pipe Rules</div>
        {onClose && (
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              color: "var(--text-2)",
              fontSize: "18px",
              cursor: "pointer",
              lineHeight: 1,
              padding: "2px 4px",
            }}
            aria-label="Close"
          >
            ×
          </button>
        )}
      </div>

      {/* Active rules */}
      <div>
        <div className="pipe-config__rules-title">Active Pipes</div>
        {rules.length === 0 ? (
          <div className="pipe-config__empty">No pipes configured yet.</div>
        ) : (
          <div className="pipe-config__rules-list">
            {rules.map((r) => {
              const from = sessions.find((s) => s.id === r.from_session_id);
              const to = sessions.find((s) => s.id === r.to_session_id);
              const toName = to?.project ?? r.to_session_id;
              const tokenState = tokens?.get(r.id);
              return (
                <div
                  key={r.id}
                  className="pipe-config__rule-item"
                  style={{ alignItems: "flex-start" }}
                >
                  <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: "4px" }}>
                    <span className="pipe-config__rule-desc">
                      <strong>{from?.project ?? r.from_session_id}</strong>
                      {" → "}
                      <strong>{toName}</strong>
                      {"  "}
                      <span style={{ color: "var(--text-2)" }}>{triggerLabel(r)}</span>
                    </span>
                    <div
                      style={{
                        fontSize: "10px",
                        color: "var(--text-2)",
                        lineHeight: 1.5,
                        fontVariantNumeric: "tabular-nums",
                      }}
                    >
                      {formatTokenBadge(tokenState, toName)}
                    </div>
                  </div>
                  <button
                    className="pipe-config__remove-btn"
                    onClick={() => handleRemove(r.id)}
                  >
                    Remove
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Add rule form */}
      <div className="pipe-config__form">
        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-from">Session A</label>
          <select
            id="pipe-from"
            className="pipe-config__select"
            value={fromId}
            onChange={(e) => setFromId(e.target.value)}
          >
            <option value="">source</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>{sessionLabel(s)}</option>
            ))}
          </select>
        </div>

        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-to">Session B</label>
          <select
            id="pipe-to"
            className="pipe-config__select"
            value={toId}
            onChange={(e) => setToId(e.target.value)}
          >
            <option value="">target</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>{sessionLabel(s)}</option>
            ))}
          </select>
        </div>

        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-trigger">Trigger</label>
          <select
            id="pipe-trigger"
            className="pipe-config__select"
            value={trigger}
            onChange={(e) => setTrigger(e.target.value as TriggerKind)}
          >
            <option value="on_idle">On Idle</option>
            <option value="on_file_write">On File Write</option>
          </select>
        </div>

        {trigger === "on_file_write" && (
          <div className="pipe-config__row">
            <label className="pipe-config__label" htmlFor="pipe-pattern">Pattern</label>
            <input
              id="pipe-pattern"
              className="pipe-config__input"
              type="text"
              placeholder="*.json"
              value={filePattern}
              onChange={(e) => setFilePattern(e.target.value)}
            />
          </div>
        )}

        {error && <div className="pipe-config__error">{error}</div>}

        <button
          className="pipe-config__add-btn"
          onClick={handleAdd}
          disabled={!fromId || !toId}
        >
          Add Pipe
        </button>
      </div>
    </div>
  );
}
