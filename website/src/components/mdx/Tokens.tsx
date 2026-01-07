import type { ReactNode } from "react";

function InlineToken({
  children,
  className,
}: {
  children: ReactNode;
  className: string;
}) {
  return (
    <code
      className={`rounded-md border border-black/10 bg-black/5 px-1.5 py-0.5 font-mono text-[0.9em] dark:border-white/10 dark:bg-white/10 ${className}`}
    >
      {children}
    </code>
  );
}

export function K({ children }: { children: ReactNode }) {
  return <InlineToken className="font-semibold">{children}</InlineToken>;
}

export function Op({ children }: { children: ReactNode }) {
  return <InlineToken className="font-semibold">{children}</InlineToken>;
}

export function Ty({ children }: { children: ReactNode }) {
  return <InlineToken className="">{children}</InlineToken>;
}

export function Ns({ children }: { children: ReactNode }) {
  return <InlineToken className="">{children}</InlineToken>;
}

export function Lit({ children }: { children: ReactNode }) {
  return <InlineToken className="">{children}</InlineToken>;
}
