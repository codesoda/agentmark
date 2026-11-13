/** Class names shared by every screen: neutral surfaces, one indigo accent. */

export const focusRing =
  "focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 focus-visible:ring-offset-2 focus-visible:ring-offset-white dark:focus-visible:ring-offset-neutral-900";

export const primaryButton =
  "inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-accent px-4 text-[13px] font-semibold text-white shadow-sm shadow-accent/20 transition-colors hover:bg-accent-hover active:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50 " +
  focusRing;

export const secondaryButton =
  "inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-lg border border-neutral-300 bg-white px-3 text-[13px] font-medium text-neutral-800 shadow-sm transition-colors hover:bg-neutral-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-white/10 dark:bg-white/[0.07] dark:text-neutral-100 dark:hover:bg-white/[0.12] " +
  focusRing;

export const linkButton =
  "rounded text-[13px] font-medium text-accent hover:text-accent-hover hover:underline dark:text-indigo-300 dark:hover:text-indigo-200 " + focusRing;

export const inputBase =
  "block w-full min-w-0 rounded-lg border bg-white px-2.5 text-[12.5px] text-neutral-900 shadow-sm shadow-black/[0.02] placeholder:font-sans placeholder:text-neutral-400 focus:outline-none focus:ring-[3px] focus:ring-accent/20 dark:bg-white/[0.06] dark:text-neutral-100 dark:placeholder:text-neutral-500";

export const inputOk = "border-neutral-300 focus:border-accent dark:border-white/10 dark:focus:border-indigo-400";
export const inputBad = "border-red-400 focus:border-red-500 dark:border-red-400/70";

export const card =
  "rounded-xl border border-neutral-200/90 bg-white shadow-sm shadow-black/[0.03] dark:border-white/[0.08] dark:bg-white/[0.04] dark:shadow-none";

export const heading = "text-[20px] font-semibold tracking-[-0.01em] text-neutral-900 dark:text-neutral-50";
export const lede = "mt-1.5 text-[13px] leading-relaxed text-neutral-600 dark:text-neutral-400";
export const muted = "text-neutral-500 dark:text-neutral-400";
export const strong = "text-neutral-900 dark:text-neutral-100";
