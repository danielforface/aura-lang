import type { ReactNode } from "react";

type Kind = "note" | "tip" | "warning" | "syntax" | "semantics";
function Box({kind,title,children}:{kind:Kind;title:string;children:ReactNode}){return <aside className={`mdx-callout ${kind}`}><strong>{title}</strong><div>{children}</div></aside>}
export function Callout({children,type="note",title="Note"}:{children:ReactNode;type?:Kind;title?:string}){return <Box kind={type} title={title}>{children}</Box>}
export function Note({children}:{children:ReactNode}){return <Box kind="note" title="Note">{children}</Box>}
export function Tip({children}:{children:ReactNode}){return <Box kind="tip" title="Tip">{children}</Box>}
export function Warning({children}:{children:ReactNode}){return <Box kind="warning" title="Warning">{children}</Box>}
export function Syntax({children}:{children:ReactNode}){return <Box kind="syntax" title="Syntax">{children}</Box>}
export function Semantics({children}:{children:ReactNode}){return <Box kind="semantics" title="Semantics">{children}</Box>}
