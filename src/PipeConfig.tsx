import { useState } from "react";
import { SessionState } from "./types";

export type PipeTriggerKind = "OnIdle" | "OnFileWrite";

export interface PipeRuleConfig {
  id: string;
  fromSessionId: string;
  toSessionId: string;
  trigger: PipeTriggerKind;
  /** Only used when trigger === "OnFileWrite". E.g. "*.json" */
  filePattern: string;
}

interface Props {
  sessions: SessionState[];
  rules: PipeRuleConfig[];
  onAddRule: (rule: PipeRuleConfig) => void;
  onRemoveRule: (id: string) => void;
}

function makeId(): string {
  return `pipe-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
}

export function PipeConfig({ sessions, rules, onAddRule, onRemoveRule }: Props) {
  const [fromId, setFromId] = useState<string>("");
  const [toId, setToId] = useState<string>("");
  const [trigger, setTrigger] = useState<PipeTriggerKind>("OnIdle");
  const [filePattern, setFilePattern] = useState<string>("*.json");
  const [error, setError] = useState<string | null>(null);

  function handleAdd() {
    setError(null);

    if (!fromId) {
      setError("Select a source session (Session A).");
      return;
    }
    if (!toId) {
      setError("Select a target session (Session B).");
      return;
    }
    if (fromId === toId) {
      setError("Source and target must be different sessions.");
      return;
    }
    if (trigger === "OnFileWrite" && !filePattern.trim()) {
      setError("Enter a file glob pattern (e.g. *.json).");
      return;
    }

    onAddRule({
      id: makeId(),
      fromSessionId: fromId,
      toSessionId: toId,
      trigger,
      filePattern: filePattern.trim(),
    });

    // Reset form but keep trigger preference
    setFromId("");
    setToId("");
    setFilePattern("*.json");
  }

  function sessionLabel(s: SessionState): string {
    return `${s.project} (${s.status})`;
  }

  return (
    <div className="pipe-config">
      <h2 className="pipe-config__title">Pipe Rules</h2>
      <p className="pipe-config__description">
        When Session A hits a trigger, automatically inject a message into
        Session B. Zero human relay.
      </p>

      {/* ── Active rules ─────────────────────────────────────────────── */}
      {rules.length > 0 && (
        <ul className="pipe-config__rules">
          {rules.map((r) => {
            const from = sessions.find((s) => s.id === r.fromSessionId);
            const to = sessions.find((s) => s.id === r.toSessionId);
            const fromLabel = from ? from.project : r.fromSessionId;
            const toLabel = to ? to.project : r.toSessionId;
            const triggerLabel =
              r.trigger === "OnIdle"
                ? "on idle"
                : `on file write (${r.filePattern})`;

            return (
              <li key={r.id} className="pipe-config__rule">
                <span className="pipe-config__rule-label">
                  <strong>{fromLabel}</strong> → <strong>{toLabel}</strong>{" "}
                  <em>{triggerLabel}</em>
                </span>
                <button
                  className="pipe-config__rule-remove"
                  onClick={() => onRemoveRule(r.id)}
                  aria-label={`Remove rule ${r.id}`}
                >
                  ✕
                </button>
              </li>
            );
          })}
        </ul>
      )}

      {rules.length === 0 && (
        <p className="pipe-config__empty">No pipes configured yet.</p>
      )}

      {/* ── Add rule form ─────────────────────────────────────────────── */}
      <div className="pipe-config__form">
        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-from">
            Session A (source)
          </label>
          <select
            id="pipe-from"
            className="pipe-config__select"
            value={fromId}
            onChange={(e) => setFromId(e.target.value)}
          >
            <option value="">— select —</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {sessionLabel(s)}
              </option>
            ))}
          </select>
        </div>

        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-to">
            Session B (target)
          </label>
          <select
            id="pipe-to"
            className="pipe-config__select"
            value={toId}
            onChange={(e) => setToId(e.target.value)}
          >
            <option value="">— select —</option>
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {sessionLabel(s)}
              </option>
            ))}
          </select>
        </div>

        <div className="pipe-config__row">
          <label className="pipe-config__label" htmlFor="pipe-trigger">
            Trigger
          </label>
          <select
            id="pipe-trigger"
            className="pipe-config__select"
            value={trigger}
            onChange={(e) => setTrigger(e.target.value as PipeTriggerKind)}
          >
            <option value="OnIdle">On Idle</option>
            <option value="OnFileWrite">On File Write</option>
          </select>
        </div>

        {trigger === "OnFileWrite" && (
          <div className="pipe-config__row">
            <label className="pipe-config__label" htmlFor="pipe-pattern">
              File pattern
            </label>
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

        {error && <p className="pipe-config__error">{error}</p>}

        <button className="pipe-config__add-btn" onClick={handleAdd}>
          Add Pipe
        </button>
      </div>
    </div>
  );
}
