import type { ReactNode } from "react";
function Token({children,className}:{children:ReactNode;className:string}){return <code className={className}>{children}</code>}
export const K=({children}:{children:ReactNode})=><Token className="tok-kw">{children}</Token>;
export const Op=({children}:{children:ReactNode})=><Token className="tok-kw">{children}</Token>;
export const Ty=({children}:{children:ReactNode})=><Token className="tok-type">{children}</Token>;
export const Ns=({children}:{children:ReactNode})=><Token className="tok-plain">{children}</Token>;
export const Lit=({children}:{children:ReactNode})=><Token className="tok-plain">{children}</Token>;
