import type { ReactNode } from "react";
export function Example({children,title="Example"}:{children:ReactNode;title?:string}){return <section className="mdx-example"><strong>{title}</strong><div>{children}</div></section>}
