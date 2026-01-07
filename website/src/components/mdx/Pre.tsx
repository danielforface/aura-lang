import type { ReactNode } from "react";
import React from "react";

function guessLanguage(children: ReactNode): string | null {
  if (!React.isValidElement(children)) return null;

  // Typical MDX output: <pre><code className="language-aura">...</code></pre>
  const codeEl = children;
  // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
  const className = (codeEl.props as any)?.className as string | undefined;
  if (!className) return null;

  const m = className.match(/language-([a-zA-Z0-9_-]+)/);
  return m?.[1] ?? null;
}

export function Pre({ children }: { children: ReactNode }) {
  const lang = guessLanguage(children);

  return (
    <div className="not-prose my-6 overflow-hidden rounded-2xl border border-black/10 bg-white/60 dark:border-white/10 dark:bg-black/40">
      <div className="flex items-center justify-between border-b border-black/10 px-4 py-2 text-xs text-zinc-600 dark:border-white/10 dark:text-zinc-300">
        <span className="font-medium">{lang ? lang.toUpperCase() : "CODE"}</span>
      </div>
      <pre className="m-0 overflow-x-auto p-4 text-sm leading-6">
        {children}
      </pre>
    </div>
  );
}
