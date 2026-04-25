interface Props {
  version: string;
  onDismiss: () => void;
}

export function UpdateBanner({ version, onDismiss }: Props) {
  const releaseUrl = `https://github.com/humos-dev/humos/releases/tag/v${version}`;

  function handleDismiss() {
    // Key on the remote version so future releases re-trigger the banner.
    localStorage.setItem(`humos-dismissed-v${version}`, "true");
    onDismiss();
  }

  return (
    <div className="update-banner">
      <div className="update-banner__left">
        <span className="update-banner__arrow">↑</span>
        <span>humOS {version} available</span>
      </div>
      <div className="update-banner__right">
        <a
          href={releaseUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="update-banner__link"
        >
          See what&apos;s new ↗
        </a>
        <button
          className="update-banner__dismiss"
          onClick={handleDismiss}
          aria-label="Dismiss update notification"
        >
          ×
        </button>
      </div>
    </div>
  );
}
