import { Example } from "./Example";
import { Pre } from "./Pre";
import { K, Lit, Ns, Op, Ty } from "./Tokens";
import { Callout, Note, Semantics, Syntax, Tip, Warning } from "./Callout";
import { KeywordTable, OperatorTable } from "./LanguageTables";

export const mdxComponents = {
  // Tag overrides
  pre: Pre,

  // Structured blocks
  Callout,
  Note,
  Tip,
  Warning,
  Syntax,
  Semantics,
  Example,

  // Inline tokens
  K,
  Op,
  Ty,
  Ns,
  Lit,

  // Tables
  KeywordTable,
  OperatorTable,
};
