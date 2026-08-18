import type { ReactNode } from "react";
import React from "react";

function languageOf(children: ReactNode) {
  if (!React.isValidElement(children)) return "code";
  const className = (children.props as { className?: string }).className;
  return className?.match(/language-([a-zA-Z0-9_-]+)/)?.[1] ?? "code";
}

export function Pre({ children }: { children: ReactNode }) {
  return <div className="mdx-pre"><div className="command-block-label">{languageOf(children)}</div><pre>{children}</pre></div>;
}
