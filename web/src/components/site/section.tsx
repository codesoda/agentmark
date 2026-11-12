import type { ReactNode } from "react";
import { cx } from "@/components/demos/kit";

export function Container({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cx("mx-auto w-full max-w-[1200px] px-5 sm:px-8", className)}>{children}</div>;
}

export function Section({
  id,
  children,
  className,
  labelledBy,
}: {
  id: string;
  children: ReactNode;
  className?: string;
  labelledBy?: string;
}) {
  return (
    <section id={id} aria-labelledby={labelledBy} className={cx("border-t border-line py-20 sm:py-28 lg:py-32", className)}>
      <Container>{children}</Container>
    </section>
  );
}

export function Eyebrow({ index, children }: { index: string; children: ReactNode }) {
  return (
    <p className="flex items-center gap-3 font-mono text-[12px] tracking-wide text-subtle">
      <span className="text-accent">{index}</span>
      <span className="h-px w-6 bg-line-strong" aria-hidden />
      <span className="uppercase tracking-[0.14em]">{children}</span>
    </p>
  );
}

export function H2({ id, children, className }: { id: string; children: ReactNode; className?: string }) {
  return (
    <h2
      id={id}
      className={cx(
        "mt-5 text-balance text-[2rem] font-semibold leading-[1.06] tracking-[-0.032em] text-ink sm:text-[2.6rem] lg:text-[3.1rem]",
        className,
      )}
    >
      {children}
    </h2>
  );
}

export function Lead({ children, className }: { children: ReactNode; className?: string }) {
  return <p className={cx("mt-5 max-w-[34rem] text-pretty text-[17px] leading-relaxed text-muted sm:text-[18px]", className)}>{children}</p>;
}

/** Short fact rows under a section lead: a bold claim, then one line of detail. */
export function Facts({ items }: { items: Array<{ title: string; body: ReactNode }> }) {
  return (
    <dl className="mt-10 max-w-[34rem] divide-y divide-line border-y border-line">
      {items.map((it) => (
        <div key={it.title} className="grid gap-1 py-4 sm:grid-cols-[10rem_1fr] sm:gap-6">
          <dt className="text-[14.5px] font-medium text-ink">{it.title}</dt>
          <dd className="text-pretty text-[14.5px] leading-relaxed text-muted">{it.body}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Inline code in running text. */
export function Code({ children }: { children: ReactNode }) {
  return (
    <code className="whitespace-nowrap rounded-[5px] bg-code-bg px-[0.35em] py-[0.1em] font-mono text-[0.86em] text-ink">{children}</code>
  );
}

/** Two-column feature layout: copy on one side, demo on the other. */
export function Feature({
  copy,
  demo,
  flip = false,
}: {
  copy: ReactNode;
  demo: ReactNode;
  flip?: boolean;
}) {
  return (
    <div
      className={cx(
        "grid items-start gap-12 lg:gap-16",
        flip ? "lg:grid-cols-[minmax(0,6fr)_minmax(0,5fr)]" : "lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)]",
      )}
    >
      <div className={cx("min-w-0 lg:pt-2", flip && "lg:order-2")}>{copy}</div>
      <div className={cx("min-w-0 lg:sticky lg:top-24", flip && "lg:order-1")}>{demo}</div>
    </div>
  );
}
