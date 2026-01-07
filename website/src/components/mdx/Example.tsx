import type { ReactNode } from "react";

export function Example({
  title = "Example",
  children,
}: {
  title?: string;
  children: ReactNode;
}) {
  return (
    <section className="not-prose my-6 rounded-2xl border border-black/10 bg-white/60 p-4 dark:border-white/10 dark:bg-black/40">
      <div className="text-xs font-semibold tracking-wide text-zinc-700 dark:text-zinc-200">
        {title}
      </div>
      <div className="mt-2">{children}</div>
    </section>
  );
}
