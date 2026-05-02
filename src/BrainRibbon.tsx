import { useState, useEffect, useRef } from "react";
import type { RelatedContext } from "./hooks/useRelatedContexts";

interface BrainRibbonProps {
  context: RelatedContext | null;
  dismissed: boolean;
  onDismiss: () => void;
  onSessionFocus?: (sessionId: string, cwd: string) => void;
}

function relativeTime(isoDate: string): string {
  const now = Date.now();
  const then = new Date(isoDate).getTime();
  const diffMs = now - then;
  const hours = Math.floor(diffMs / 3_600_000);
  if (hours < 1) return "just now";
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days === 1) return "yesterday";
  if (days < 7) return `${days}d`;
  return `${Math.floor(days / 7)}w`;
}

export function BrainRibbon({ context, dismissed, onDismiss, onSessionFocus }: BrainRibbonProps) {
  const [expanded, setExpanded] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);

  // Reset expanded when context changes
  useEffect(() => {
    setExpanded(false);
  }, [context?.cwd]);

  // Null context = loading
  if (context === null) {
    return (
      <div className="brain-ribbon brain-ribbon--loading" aria-live="polite">
        <div className="brain-ribbon__trigger">
          <span className="brain-ribbon__glyph">{"\u25B8_"}</span>
          <div className="brain-ribbon__skeleton" />
        </div>
      </div>
    );
  }

  // Daemon offline: suppressed per product decision (show nothing until auto-start ships)
  if (!context.daemon_online) return null;

  // Zero matches or dismissed: no ribbon
  if (context.matches.length === 0 || dismissed) return null;

  const totalCount = context.total_count;
  const isStale = context.is_stale;
  const isMany = totalCount > 9;

  const ribbonClass = [
    "brain-ribbon",
    isStale ? "brain-ribbon--stale" : "",
    expanded ? "brain-ribbon--expanded" : "",
  ].filter(Boolean).join(" ");

  const countText = totalCount === 1
    ? "1 related session"
    : `${totalCount} related sessions`;

  const subtitleText = isStale
    ? "updating..."
    : "in this directory";

  function handleTriggerClick() {
    setExpanded((v) => !v);
  }

  function handleTriggerKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setExpanded((v) => !v);
    }
    if (e.key === "Escape" && expanded) {
      e.preventDefault();
      setExpanded(false);
    }
  }

  function handleListKeyDown(e: React.KeyboardEvent) {
    const list = listRef.current;
    if (!list) return;
    const items = Array.from(list.querySelectorAll<HTMLElement>("[tabindex]"));
    const idx = items.indexOf(e.target as HTMLElement);
    if (e.key === "ArrowDown" && idx < items.length - 1) {
      e.preventDefault();
      items[idx + 1].focus();
    }
    if (e.key === "ArrowUp" && idx > 0) {
      e.preventDefault();
      items[idx - 1].focus();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      setExpanded(false);
    }
  }

  if (expanded) {
    return (
      <div className={ribbonClass} aria-live="polite">
        <div className="brain-ribbon__header-wrap">
          <div
            className="brain-ribbon__trigger"
            role="button"
            aria-expanded="true"
            tabIndex={0}
            onClick={handleTriggerClick}
            onKeyDown={handleTriggerKeyDown}
          >
            <span className="brain-ribbon__glyph">{"\u25B8_"}</span>
            <span className="brain-ribbon__text">
              <strong>{countText}</strong> {subtitleText}
            </span>
            <span className="brain-ribbon__chev">{"\u25BE"}</span>
          </div>
          <span className="brain-ribbon__sep" />
          <button
            className="brain-ribbon__dismiss"
            aria-label="Collapse"
            onClick={() => setExpanded(false)}
          >
            {"\u25B4"}
          </button>
        </div>
        <ul
          className="brain-ribbon__list"
          role="list"
          ref={listRef}
          onKeyDown={handleListKeyDown}
        >
          {context.matches.slice(0, 5).map((m) => (
            <li
              key={m.session_id}
              className="brain-ribbon__item"
              tabIndex={0}
              onClick={() => onSessionFocus?.(m.session_id, m.cwd)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onSessionFocus?.(m.session_id, m.cwd);
              }}
            >
              <span className="brain-ribbon__item-snippet">{m.snippet}</span>
              <span className="brain-ribbon__item-meta">{relativeTime(m.modified_at)}</span>
            </li>
          ))}
        </ul>
        {totalCount > 5 && (
          <div className="brain-ribbon__view-all">
            and {totalCount - 5} more
          </div>
        )}
      </div>
    );
  }

  // Collapsed (default) state
  return (
    <div className={ribbonClass} aria-live="polite">
      <div
        className="brain-ribbon__trigger"
        role="button"
        aria-expanded="false"
        tabIndex={0}
        onClick={handleTriggerClick}
        onKeyDown={handleTriggerKeyDown}
      >
        <span className={`brain-ribbon__glyph${isMany ? " brain-ribbon__glyph--pulse" : ""}`}>
          {"\u25B8_"}
        </span>
        <span className="brain-ribbon__text">
          <strong>{countText}</strong> {subtitleText}
        </span>
        <span className="brain-ribbon__chev">{"\u25BE"}</span>
      </div>
      <span className="brain-ribbon__sep" />
      <button
        className="brain-ribbon__dismiss"
        aria-label="Hide related sessions for this session"
        onClick={(e) => { e.stopPropagation(); onDismiss(); }}
      >
        {"\u00D7"}
      </button>
    </div>
  );
}
