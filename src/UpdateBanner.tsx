import { useState } from "react";

interface Props {
  version: string;
  releaseUrl: string;
  onDismiss: () => void;
}

const INSTALL_CMD = "curl -fsSL https://humos.dev/install.sh | sh";

export function UpdateBanner({ version, releaseUrl, onDismiss }: Props) {
  const [copied, setCopied] = useState(false);

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

  return (
    <div className="update-banner">
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
    </div>
  );
}
