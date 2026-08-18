import type { ReactNode } from "react";

export function CodePanel({
  label,
  meta,
  children,
  footer,
}: {
  label?: string;
  meta?: string;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className="code-panel">
      {(label || meta) ? (
        <div className="code-panel-head">
          <div className="window-dots" aria-hidden="true"><span /><span /><span /></div>
          <span>{label}</span>
          {meta ? <span className="code-panel-meta">{meta}</span> : null}
        </div>
      ) : null}
      <div className="code-panel-body">{children}</div>
      {footer ? <div className="code-panel-footer">{footer}</div> : null}
    </div>
  );
}
