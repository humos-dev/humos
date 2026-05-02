import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Props {
  version: string;
  releaseUrl: string;
  onDismiss: () => void;
}

type UpdateStage =
  | { stage: "idle" }
  | { stage: "checking" }
  | { stage: "downloading"; progress: number }
  | { stage: "installing" }
  | { stage: "ready"; canAutoRestart: boolean }
  | { stage: "error"; error: string };

const INSTALL_CMD = "curl -fsSL https://humos.dev/install.sh | sh";

export function UpdateBanner({ version, releaseUrl, onDismiss }: Props) {
  const [copied, setCopied] = useState(false);
  const [updateStage, setUpdateStage] = useState<UpdateStage>({ stage: "idle" });

  useEffect(() => {
    const unlisten = listen<UpdateStage>("update:state", (event) => {
      setUpdateStage(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  function handleDismiss() {
    localStorage.setItem(`humos-dismissed-v${version}`, "true");
    onDismiss();
  }

  function handleCopy() {
    navigator.clipboard.writeText(INSTALL_CMD).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => {});
  }

  const handleUpdate = useCallback(() => {
    setUpdateStage({ stage: "checking" });
    invoke("start_self_update").catch(() => {
      setUpdateStage({ stage: "error", error: "Failed to start update" });
    });
  }, []);

  const handleRestart = useCallback(() => {
    invoke("restart_app").catch(() => {});
  }, []);

  const stage = updateStage.stage;

  return (
    <div className="update-banner" style={{ position: "relative" }}>
      {/* idle */}
      {stage === "idle" && (
        <>
          <div className="update-banner__left">
            <span className="update-banner__arrow">&#x2191;</span>
            <span>humOS {version} available</span>
          </div>
          <div className="update-banner__cmd">
            <code className="update-banner__cmd-text">$ {INSTALL_CMD}</code>
            <button
              className="update-banner__copy"
              onClick={handleCopy}
              aria-label="Copy install command"
              aria-live="polite"
            >
              {copied ? "Copied!" : "Copy"}
            </button>
            <button
              className="update-banner__trigger"
              onClick={handleUpdate}
              aria-label="Start update"
            >
              Update &#x25B6;
            </button>
          </div>
          <div className="update-banner__right">
            <a
              href={releaseUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="update-banner__link"
            >
              See what&apos;s new &#x2197;
            </a>
            <button
              className="update-banner__dismiss"
              onClick={handleDismiss}
              aria-label="Dismiss update notification"
            >
              &#xD7;
            </button>
          </div>
        </>
      )}

      {/* checking */}
      {stage === "checking" && (
        <>
          <div className="update-banner__left">
            <span className="update-banner__arrow">&#x2191;</span>
            <span>humOS {version} available</span>
          </div>
          <div className="update-banner__cmd">
            <span className="update-banner__stage">checking...</span>
          </div>
          <div className="update-banner__right">
            <button
              className="update-banner__dismiss"
              onClick={handleDismiss}
              aria-label="Dismiss update notification"
            >
              &#xD7;
            </button>
          </div>
        </>
      )}

      {/* downloading */}
      {stage === "downloading" && (
        <>
          <div className="update-banner__left">
            <span className="update-banner__arrow">&#x2191;</span>
            <span>humOS {version} available</span>
          </div>
          <div className="update-banner__cmd">
            <span
              className="update-banner__stage"
              style={{ color: "var(--coord)", fontVariantNumeric: "tabular-nums" }}
            >
              {(updateStage as { stage: "downloading"; progress: number }).progress}%
            </span>
          </div>
          <div className="update-banner__right">
            <button
              className="update-banner__dismiss"
              onClick={handleDismiss}
              aria-label="Dismiss update notification"
            >
              &#xD7;
            </button>
          </div>
          <div
            className="update-banner__progress"
            role="progressbar"
            aria-valuenow={(updateStage as { stage: "downloading"; progress: number }).progress}
            aria-valuemin={0}
            aria-valuemax={100}
            style={{ width: `${(updateStage as { stage: "downloading"; progress: number }).progress}%` }}
          />
        </>
      )}

      {/* installing */}
      {stage === "installing" && (
        <>
          <div className="update-banner__left">
            <span className="update-banner__arrow">&#x2191;</span>
            <span>humOS {version} available</span>
          </div>
          <div className="update-banner__cmd">
            <span className="update-banner__stage">installing...</span>
          </div>
          <div className="update-banner__right" />
          <div
            className="update-banner__progress update-banner__progress--indeterminate"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
          />
        </>
      )}

      {/* ready */}
      {stage === "ready" && (
        <>
          <div className="update-banner__left" style={{ color: "#3ecf8e" }}>
            <span>&#x2713; humOS {version} ready</span>
          </div>
          <div className="update-banner__cmd" />
          <div className="update-banner__right">
            <button
              className="update-banner__restart"
              onClick={handleRestart}
              aria-label="Restart humOS to apply update"
            >
              Restart humOS
            </button>
            <button
              className="update-banner__dismiss"
              onClick={handleDismiss}
              aria-label="Dismiss update notification"
            >
              &#xD7;
            </button>
          </div>
        </>
      )}

      {/* error */}
      {stage === "error" && (
        <>
          <div className="update-banner__left" style={{ color: "#f87171" }}>
            <span>&#x21; Update failed</span>
          </div>
          <div className="update-banner__cmd">
            <span className="update-banner__stage">
              {(updateStage as { stage: "error"; error: string }).error}
            </span>
          </div>
          <div className="update-banner__right">
            <a
              href={releaseUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="update-banner__link"
            >
              Try manually &#x2197;
            </a>
            <button
              className="update-banner__dismiss"
              onClick={handleDismiss}
              aria-label="Dismiss update notification"
            >
              &#xD7;
            </button>
          </div>
        </>
      )}
    </div>
  );
}
