import AgentMarkMark from "./AgentMarkMark";
import CopyCommand from "./CopyCommand";
import { detectPlatform, installCommandFor } from "./platforms";
import { runtimeExtensionId, usePlatformInfo } from "./usePlatformInfo";

/**
 * The app answered "unknown message type" to a setup message: it predates
 * this extension (contract §4.2). Shows the install command for this
 * platform, which updates in place.
 */
export default function UpdateRequired({ onCheckAgain, version }: { onCheckAgain: () => void; version?: string }) {
  const info = usePlatformInfo();
  const command = info === undefined ? undefined : installCommandFor(detectPlatform(info ?? undefined)?.key, runtimeExtensionId());

  return (
    <section aria-labelledby="update-required-heading" data-testid="update-required">
      <div className="flex items-center gap-2">
        <AgentMarkMark look="idle" />
        <h1 id="update-required-heading" className="text-[15px] font-semibold text-gray-900">
          Update AgentMark
        </h1>
      </div>
      <p className="mt-1.5 text-[13px] leading-snug text-gray-600">
        This AgentMark app{version ? ` (${version})` : ""} is too old for this extension. Update it:
      </p>
      {command ? (
        <>
          <p className="mt-3 text-[13px] text-gray-800">Open Terminal and paste this:</p>
          <CopyCommand command={command} label="update command" />
        </>
      ) : (
        info !== undefined && (
          <p className="mt-3 text-[13px] text-gray-600">Install the latest AgentMark for this computer.</p>
        )
      )}
      <button
        type="button"
        onClick={onCheckAgain}
        className="mt-4 rounded-md bg-indigo-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1"
        data-testid="install-retry"
      >
        Check again
      </button>
    </section>
  );
}
