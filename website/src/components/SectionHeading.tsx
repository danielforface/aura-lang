import type { ReactNode } from "react";

export function SectionHeading({
  eyebrow,
  title,
  body,
  aside,
}: {
  eyebrow: string;
  title: string;
  body?: string;
  aside?: ReactNode;
}) {
  return (
    <div className="section-heading">
      <div>
        <div className="eyebrow">{eyebrow}</div>
        <h2>{title}</h2>
        {body ? <p>{body}</p> : null}
      </div>
      {aside ? <div className="section-heading-aside">{aside}</div> : null}
    </div>
  );
}
