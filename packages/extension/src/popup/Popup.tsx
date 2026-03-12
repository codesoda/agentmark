import { useEffect, useRef, useState } from "react";
import {
  queryActiveTab,
  sendSaveMessage,
  isConnectionError,
} from "../shared/runtime";
import type { SaveResultResponse } from "../shared/types";

type PopupState =
  | { status: "saving"; title?: string; favIconUrl?: string }
  | {
      status: "success";
      title?: string;
      favIconUrl?: string;
      bookmarkId: string;
      saveStatus: string;
    }
  | { status: "error"; message: string; isConnectionError: boolean };

const AUTO_CLOSE_MS = 2000;

export default function Popup() {
  const [state, setState] = useState<PopupState>({
    status: "saving",
  });
  const dispatched = useRef(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (dispatched.current) return;
    dispatched.current = true;

    (async () => {
      let tab;
      try {
        tab = await queryActiveTab();
      } catch (err) {
        setState({
          status: "error",
          message: err instanceof Error ? err.message : String(err),
          isConnectionError: false,
        });
        return;
      }

      setState({ status: "saving", title: tab.title, favIconUrl: tab.favIconUrl });

      const response = await sendSaveMessage(tab.url, tab.title);

      if (!response.success) {
        setState({
          status: "error",
          message: response.error,
          isConnectionError: isConnectionError(response.error),
        });
        return;
      }

      const data = response.data as SaveResultResponse;
      setState({
        status: "success",
        title: tab.title,
        favIconUrl: tab.favIconUrl,
        bookmarkId: data.id,
        saveStatus: data.status,
      });
    })();

    return () => {
      if (closeTimer.current) {
        clearTimeout(closeTimer.current);
      }
    };
  }, []);

  // Auto-close on success
  useEffect(() => {
    if (state.status === "success") {
      closeTimer.current = setTimeout(() => {
        window.close();
      }, AUTO_CLOSE_MS);

      return () => {
        if (closeTimer.current) {
          clearTimeout(closeTimer.current);
          closeTimer.current = null;
        }
      };
    }
  }, [state.status]);

  const handleMoreOptions = () => {
    // Seam for Spec 21: extended save form will replace this
  };

  return (
    <div className="w-80 p-4">
      {state.status === "saving" && (
        <div data-testid="saving-state">
          <div className="flex items-center gap-2">
            {state.favIconUrl && (
              <img
                src={state.favIconUrl}
                alt=""
                className="h-4 w-4"
              />
            )}
            <h1 className="text-lg font-semibold text-gray-900">Saving...</h1>
          </div>
          {state.title && (
            <p className="mt-1 truncate text-sm text-gray-600">{state.title}</p>
          )}
        </div>
      )}

      {state.status === "success" && (
        <div data-testid="success-state">
          <div className="flex items-center gap-2">
            {state.favIconUrl && (
              <img
                src={state.favIconUrl}
                alt=""
                className="h-4 w-4"
              />
            )}
            <h1 className="text-lg font-semibold text-green-700">Saved!</h1>
          </div>
          {state.title && (
            <p className="mt-1 truncate text-sm text-gray-600">{state.title}</p>
          )}
          <p className="mt-1 text-xs text-gray-400">
            ID: {state.bookmarkId}
          </p>
          <button
            type="button"
            onClick={handleMoreOptions}
            className="mt-2 text-xs text-indigo-600 hover:text-indigo-800"
          >
            More options
          </button>
        </div>
      )}

      {state.status === "error" && (
        <div data-testid="error-state">
          <h1 className="text-lg font-semibold text-red-700">Save Failed</h1>
          <p className="mt-1 text-sm text-gray-600">{state.message}</p>
          {state.isConnectionError && (
            <div className="mt-3 rounded border border-amber-200 bg-amber-50 p-2 text-xs text-amber-800" data-testid="install-guidance">
              <p className="font-medium">Native host not connected</p>
              <p className="mt-1">
                Install the AgentMark CLI and register the native messaging host:
              </p>
              <ol className="mt-1 list-inside list-decimal space-y-0.5">
                <li>Install: <code>cargo install agentmark</code></li>
                <li>Register: <code>agentmark install-host</code></li>
                <li>Reload this extension</li>
              </ol>
            </div>
          )}
          <button
            type="button"
            onClick={handleMoreOptions}
            className="mt-2 text-xs text-indigo-600 hover:text-indigo-800"
          >
            More options
          </button>
        </div>
      )}
    </div>
  );
}
