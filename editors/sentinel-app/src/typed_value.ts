// TypedValue Interface for Explain Panel

export type TypedValueKind =
  | 'Primitive'
  | 'Record'
  | 'Enum'
  | 'Array'
  | 'Reference'
  | 'Tuple'
  | 'Function'
  | 'Unknown';

export interface TypedValue {
  kind: TypedValueKind;
  typ?: string;
  value?: string;
  name?: string;
  fields?: Record<string, TypedValue>;
  variant?: string;
  payload?: TypedValue;
  elementType?: string;
  elements?: TypedValue[];
  referent?: TypedValue;
  mutable?: boolean;
  arity?: number;
  reason?: string;
  fallbackJson?: string;
}
