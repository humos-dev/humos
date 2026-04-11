import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SessionState } from "./types";

type TriggerKind = "on_idle" | "on_file_write";

interface PipeRule {
  id: string;
  from_session_id: string;
  to_session_id: string;
  trigger: { OnIdle: null } | { OnFileWrite: string };
}

interface Props {
  sessions: SessionState[];
}

function triggerLabel(rule: PipeRule): string {
  if ("OnIdle" in rule.trigger) return "on idle";
  if ("OnFileWrite" in rule.trigger) return `on file write (${rule.trigger.OnFileWrite})`;
  return "unknown";
}

export function PipeConfig({ sessions }: Props) {
  const [rules, setRules] = useState<PipeRule[]>([]);
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [trigger, setTrigger] = useState<TriggerKind>("on_idle");
  const [filePattern, setFilePattern] = useState("*.json");
  const [error, setError] = useState<string | null>(null);

  async function loadRules() {
    try {
      const result = await invoke<PipeRule[]>("list_pipe_rules");
      setRules(result);
    } catch (err) {
      console.error("list_pipe_rules error:", err);
    }
  }

  useEffect(() => {
    loadRules();
  }, []);

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
      await invoke("add_pipe_rule", {
        fromSessionId: fromId,
        toSessionId: toId,
        trigger,
        filePattern: trigger === "on_file_write" ? filePattern.trim() : null,
      });
      setFromId("");
      setToId("");
      setFilePattern("*.json");
      await loadRules();
    } catch (err) {
      setError(`Error: ${err}`);
    }
  }

  async function handleRemove(id: string) {
    try {
      await invoke("remove_pipe_rule", { ruleId: id });
      await loadRules();
    } catch (err) {
      console.error("remove_pipe_rule error:", err);
    }
  }

  function sessionLabel(s: SessionState) {
    return `${s.project} (${s.status})`;
  }

  return (
    <div className="pipe-config__drawer">
      <div className="pipe-config__title">Pipe Rules</div>

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
              return (
                <div key={r.id} className="pipe-config__rule-item">
                  <span className="pipe-config__rule-desc">
                    <strong>{from?.project ?? r.from_session_id}</strong>
                    {" → "}
                    <strong>{to?.project ?? r.to_session_id}</strong>
                    {"  "}
                    <span style={{ color: "var(--text-2)" }}>{triggerLabel(r)}</span>
                  </span>
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
            <option value="">— source —</option>
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
            <option value="">— target —</option>
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
