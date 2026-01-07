import type { ReactNode } from "react";

type CalloutVariant = "note" | "tip" | "warning" | "syntax" | "semantics";

const variantLabel: Record<CalloutVariant, string> = {
  note: "Note",
  tip: "Tip",
  warning: "Warning",
  syntax: "Syntax",
  semantics: "Semantics",
};

function variantClasses(variant: CalloutVariant) {
  switch (variant) {
    case "warning":
      return "border-black/20 bg-black/5 dark:border-white/20 dark:bg-white/5";
    case "tip":
      return "border-black/10 bg-black/5 dark:border-white/10 dark:bg-white/5";
    case "syntax":
      return "border-black/10 bg-white/60 dark:border-white/10 dark:bg-black/40";
    case "semantics":
      return "border-black/10 bg-white/60 dark:border-white/10 dark:bg-black/40";
    case "note":
    default:
      return "border-black/10 bg-white/60 dark:border-white/10 dark:bg-black/40";
  }
}

export function Callout({
  variant = "note",
  title,
  children,
}: {
  variant?: CalloutVariant;
  title?: string;
  children: ReactNode;
}) {
  return (
    <section
      className={`not-prose my-6 rounded-2xl border p-4 ${variantClasses(variant)}`}
    >
      <div className="text-xs font-semibold tracking-wide text-zinc-700 dark:text-zinc-200">
        {title ?? variantLabel[variant]}
      </div>
      <div className="mt-2 text-sm leading-6 text-zinc-800 dark:text-zinc-200">
        {children}
      </div>
    </section>
  );
}

export function Note(props: { title?: string; children: ReactNode }) {
  return <Callout variant="note" {...props} />;
}

export function Tip(props: { title?: string; children: ReactNode }) {
  return <Callout variant="tip" {...props} />;
}

export function Warning(props: { title?: string; children: ReactNode }) {
  return <Callout variant="warning" {...props} />;
}

export function Syntax(props: { title?: string; children: ReactNode }) {
  return <Callout variant="syntax" {...props} />;
}

export function Semantics(props: { title?: string; children: ReactNode }) {
  return <Callout variant="semantics" {...props} />;
}
