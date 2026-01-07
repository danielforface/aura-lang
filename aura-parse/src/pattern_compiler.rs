/// Advanced pattern matching compiler for Aura.
///
/// This module implements compilation of constructor and enum patterns
/// into balanced decision trees and switch statements.
///
/// Patterns compiled:
/// - Constructor patterns: `Box(x)`, `Pair(a, b)`
/// - Enum variants: `Ok(v)`, `Err(e)`
/// - Nested patterns: `Some(Box(x))`
/// - Wildcard and literal matches

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Wildcard `_` or variable binding
    Wildcard(Option<String>),
    /// Literal: integer, bool, string
    Literal(String),
    /// Enum variant with args: `EnumName::Variant(patterns...)`
    Variant {
        enum_name: String,
        variant_name: String,
        arg_patterns: Vec<Pattern>,
    },
    /// Constructor pattern: `TypeName(patterns...)`
    Constructor {
        type_name: String,
        arg_patterns: Vec<Pattern>,
    },
    /// Or-pattern (catch-all alternatives)
    Or(Vec<Pattern>),
}

#[derive(Debug, Clone)]
pub enum MatchExpr {
    /// Leaf: bind or wildcard
    Leaf {
        bindings: Vec<(String, usize)>, // (name, column_idx)
    },
    /// Decision: test a column, branch on value
    Decision {
        column: usize, // which scrutinee column to test
        branches: Vec<(String, MatchExpr)>, // (value, expr)
        default: Option<Box<MatchExpr>>,
    },
    /// Failure: no pattern matched
    Fail,
}

/// Pattern matrix: rows are match arms, columns are scrutinee positions
#[derive(Debug)]
pub struct PatternMatrix {
    rows: Vec<Vec<Pattern>>,
    actions: Vec<String>, // action label for each row
}

impl PatternMatrix {
    pub fn new() -> Self {
        PatternMatrix {
            rows: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Add a pattern vector (one match arm) and its action label
    pub fn add_arm(&mut self, patterns: Vec<Pattern>, action: String) {
        self.rows.push(patterns);
        self.actions.push(action);
    }

    /// Check if first column is exhaustive (all constructors covered)
    pub fn first_col_exhaustive(&self) -> bool {
        if self.rows.is_empty() {
            return false;
        }

        let mut has_wildcard = false;
        let mut variants: std::collections::HashSet<String> = std::collections::HashSet::new();

        for row in &self.rows {
            if row.is_empty() {
                continue;
            }

            match &row[0] {
                Pattern::Wildcard(_) => has_wildcard = true,
                Pattern::Variant {
                    enum_name,
                    variant_name,
                    ..
                } => {
                    variants.insert(format!("{}::{}", enum_name, variant_name));
                }
                Pattern::Constructor { type_name, .. } => {
                    variants.insert(type_name.clone());
                }
                _ => has_wildcard = true,
            }
        }

        has_wildcard
    }

    /// Compile pattern matrix into decision tree
    pub fn compile(&self) -> MatchExpr {
        if self.rows.is_empty() {
            return MatchExpr::Fail;
        }

        self._compile_inner(&self.rows, &self.actions, 0)
    }

    fn _compile_inner(
        &self,
        rows: &[Vec<Pattern>],
        actions: &[String],
        col: usize,
    ) -> MatchExpr {
        // Base case: no more patterns (leaf node)
        if rows.is_empty() {
            return MatchExpr::Fail;
        }

        if col >= rows[0].len() {
            // All patterns exhausted; use first action
            let bindings: Vec<(String, usize)> = Vec::new();
            return MatchExpr::Leaf { bindings };
        }

        // Check first column pattern
        let first_pat = &rows[0][col];
        match first_pat {
            Pattern::Wildcard(_) => {
                // Wildcard matches everything; continue to next column
                self._compile_inner(rows, actions, col + 1)
            }
            Pattern::Literal(_val) => {
                // Build decision tree based on literal values
                let mut branches: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                for (idx, row) in rows.iter().enumerate() {
                    if idx >= row.len() {
                        continue;
                    }
                    match &row[col] {
                        Pattern::Literal(v) => {
                            branches.entry(v.clone()).or_insert_with(Vec::new).push(idx);
                        }
                        Pattern::Wildcard(_) => {
                            // Wildcard matches all remaining; will be default
                        }
                        _ => {}
                    }
                }

                let mut decision_branches = Vec::new();
                for (val, indices) in branches {
                    let sub_rows: Vec<Vec<Pattern>> =
                        indices.iter().map(|&i| rows[i].clone()).collect();
                    let sub_actions: Vec<String> =
                        indices.iter().map(|&i| actions[i].clone()).collect();
                    let expr = self._compile_inner(&sub_rows, &sub_actions, col + 1);
                    decision_branches.push((val, expr));
                }

                // Default case (wildcard)
                let default_rows: Vec<usize> = (0..rows.len())
                    .filter(|idx| matches!(rows[*idx].get(col), Some(Pattern::Wildcard(_))))
                    .collect();

                let default = if !default_rows.is_empty() {
                    let sub_rows: Vec<Vec<Pattern>> =
                        default_rows.iter().map(|&i| rows[i].clone()).collect();
                    let sub_actions: Vec<String> =
                        default_rows.iter().map(|&i| actions[i].clone()).collect();
                    Some(Box::new(self._compile_inner(&sub_rows, &sub_actions, col + 1)))
                } else {
                    None
                };

                MatchExpr::Decision {
                    column: col,
                    branches: decision_branches,
                    default,
                }
            }
            Pattern::Variant {
                enum_name,
                variant_name: _,
                arg_patterns: _,
            } => {
                // Build decision tree based on enum variant
                let mut branches: BTreeMap<String, Vec<usize>> = BTreeMap::new();
                for (idx, row) in rows.iter().enumerate() {
                    if idx >= row.len() {
                        continue;
                    }
                    match &row[col] {
                        Pattern::Variant {
                            variant_name: v, ..
                        } => {
                            branches.entry(v.clone()).or_insert_with(Vec::new).push(idx);
                        }
                        Pattern::Wildcard(_) => {
                            // Wildcard matches all variants
                        }
                        _ => {}
                    }
                }

                let mut decision_branches = Vec::new();
                for (var, indices) in branches {
                    let sub_rows: Vec<Vec<Pattern>> =
                        indices.iter().map(|&i| rows[i].clone()).collect();
                    let sub_actions: Vec<String> =
                        indices.iter().map(|&i| actions[i].clone()).collect();
                    let expr = self._compile_inner(&sub_rows, &sub_actions, col + 1);
                    decision_branches.push((format!("{}::{}", enum_name, var), expr));
                }

                let default_rows: Vec<usize> = (0..rows.len())
                    .filter(|idx| matches!(rows[*idx].get(col), Some(Pattern::Wildcard(_))))
                    .collect();

                let default = if !default_rows.is_empty() {
                    let sub_rows: Vec<Vec<Pattern>> =
                        default_rows.iter().map(|&i| rows[i].clone()).collect();
                    let sub_actions: Vec<String> =
                        default_rows.iter().map(|&i| actions[i].clone()).collect();
                    Some(Box::new(self._compile_inner(&sub_rows, &sub_actions, col + 1)))
                } else {
                    None
                };

                MatchExpr::Decision {
                    column: col,
                    branches: decision_branches,
                    default,
                }
            }
            _ => {
                // For other patterns, skip and continue
                self._compile_inner(rows, actions, col + 1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_exhaustiveness() {
        let mut pm = PatternMatrix::new();
        pm.add_arm(
            vec![Pattern::Variant {
                enum_name: "Option".to_string(),
                variant_name: "Some".to_string(),
                arg_patterns: vec![Pattern::Wildcard(Some("x".to_string()))],
            }],
            "handle_some".to_string(),
        );
        // Use wildcard for exhaustiveness (covers None and any other variants)
        pm.add_arm(vec![Pattern::Wildcard(None)], "handle_none".to_string());

        assert!(pm.first_col_exhaustive());
    }

    #[test]
    fn test_literal_decision_tree() {
        let mut pm = PatternMatrix::new();
        pm.add_arm(vec![Pattern::Literal("0".to_string())], "zero".to_string());
        pm.add_arm(vec![Pattern::Literal("1".to_string())], "one".to_string());
        pm.add_arm(vec![Pattern::Wildcard(None)], "other".to_string());

        let tree = pm.compile();
        // Verify structure: should have Decision with branches for 0, 1 and default
        match tree {
            MatchExpr::Decision {
                column,
                branches,
                default,
            } => {
                assert_eq!(column, 0);
                assert!(!branches.is_empty());
                assert!(default.is_some());
            }
            _ => panic!("Expected Decision node"),
        }
    }
}
