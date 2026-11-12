import type { ReactNode } from "react";

/** A screen: scrolling content above an optional fixed footer for its actions. */
export default function Screen({ children, footer, testId }: { children: ReactNode; footer?: ReactNode; testId?: string }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-testid={testId}>
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6 pt-5">{children}</div>
      {footer && (
        <div className="shrink-0 border-t border-neutral-200/80 bg-neutral-50/95 px-6 py-3.5 dark:border-white/[0.07] dark:bg-neutral-900/95">
          {footer}
        </div>
      )}
    </div>
  );
}
