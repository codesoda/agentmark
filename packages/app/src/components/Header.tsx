import AgentMarkMark from "./AgentMarkMark";
import { focusRing } from "./ui";

export type View = "home" | "setup" | "extension" | "settings" | "about";

/** On macOS the window's traffic lights sit over the left of the header. */
const IS_MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.platform || navigator.userAgent);

const TABS: { view: View; label: string; needsSetup: boolean }[] = [
  { view: "home", label: "Home", needsSetup: false },
  { view: "settings", label: "Settings", needsSetup: true },
  { view: "about", label: "About", needsSetup: false },
];

/** Setup and the extension steps are part of Home. */
function tabFor(view: View): View {
  return view === "setup" || view === "extension" ? "home" : view;
}

export default function Header({
  view,
  onNavigate,
  tabs,
}: {
  view: View;
  onNavigate: (view: View) => void;
  /** Settings is offered once AgentMark is set up. */
  tabs: boolean;
}) {
  const current = tabFor(view);
  return (
    <header
      data-tauri-drag-region
      className={`flex h-[52px] shrink-0 select-none items-center justify-between gap-3 border-b border-neutral-200/80 bg-neutral-50/80 pr-3 dark:border-white/[0.07] dark:bg-neutral-900 ${
        IS_MAC ? "pl-[92px]" : "pl-4"
      }`}
    >
      <div data-tauri-drag-region className="flex min-w-0 items-center gap-2">
        <AgentMarkMark size={18} />
        <span data-tauri-drag-region className="text-[13px] font-semibold tracking-[-0.005em] text-neutral-900 dark:text-neutral-100">
          AgentMark
        </span>
      </div>
      <nav aria-label="Sections" className="flex items-center gap-0.5 rounded-lg bg-neutral-200/60 p-0.5 dark:bg-white/[0.06]">
        {TABS.filter((t) => tabs || !t.needsSetup).map((tab) => {
          const selected = tab.view === current;
          return (
            <button
              key={tab.view}
              type="button"
              onClick={() => onNavigate(tab.view)}
              aria-current={selected ? "page" : undefined}
              className={`h-[26px] rounded-md px-2.5 text-xs font-medium transition-colors ${focusRing} ${
                selected
                  ? "bg-white text-neutral-900 shadow-sm shadow-black/10 dark:bg-white/[0.14] dark:text-white dark:shadow-none"
                  : "text-neutral-600 hover:text-neutral-900 dark:text-neutral-400 dark:hover:text-neutral-100"
              }`}
              data-testid={`tab-${tab.view}`}
            >
              {tab.label}
            </button>
          );
        })}
      </nav>
    </header>
  );
}
