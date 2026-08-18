import { Example } from "./Example";
import { Pre } from "./Pre";
import { K, Lit, Ns, Op, Ty } from "./Tokens";
import { Callout, Note, Semantics, Syntax, Tip, Warning } from "./Callout";
import { KeywordTable, OperatorTable } from "./LanguageTables";
export const mdxComponents = { pre: Pre, Callout, Note, Tip, Warning, Syntax, Semantics, Example, K, Op, Ty, Ns, Lit, KeywordTable, OperatorTable };
