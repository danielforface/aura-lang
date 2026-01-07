/// Jump-table lowering for dense match expressions.
///
/// When a match has many integer literal patterns, it's more efficient to
/// generate a jump table (LLVM switch) than cascading if/else branches.
///
/// This module detects dense patterns and lowers them to jump tables.

use std::collections::{BTreeMap, BTreeSet};

/// Represents a match arm with a pattern and action.
#[derive(Clone, Debug)]
pub struct MatchArm {
    pub pattern: PatternValue,
    pub action_id: u32,
}

/// A value that can be matched on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatternValue {
    IntLiteral(i64),
    StringLiteral(String),
    Variant(String),
}

/// Metrics for determining if a jump table is beneficial.
#[derive(Clone, Debug)]
pub struct JumpTableMetrics {
    /// Number of distinct literal values
    pub pattern_count: usize,
    /// Range from min to max value (for dense detection)
    pub value_range: usize,
    /// Density = patterns / range. >0.5 = dense, use jump table
    pub density: f64,
    /// Estimated code size for jump table vs cascading if/else
    pub jump_table_bytes: usize,
    pub cascade_bytes: usize,
}

impl JumpTableMetrics {
    /// Determine if a jump table would be more efficient.
    pub fn should_use_jump_table(&self) -> bool {
        self.density > 0.5 && self.jump_table_bytes < self.cascade_bytes
    }
}

/// Jump table entry: value -> action ID
#[derive(Clone, Debug)]
pub struct JumpTableEntry {
    pub value: i64,
    pub action_id: u32,
}

/// A jump table for O(1) dispatch on literal values.
#[derive(Clone, Debug)]
pub struct JumpTable {
    pub base: i64,
    pub size: usize,
    /// entries[i] corresponds to value (base + i)
    pub entries: Vec<Option<u32>>,
    pub default_action: u32,
}

impl JumpTable {
    /// Create a jump table from a set of match arms with integer literals.
    pub fn from_arms(arms: &[MatchArm], default_action: u32) -> Option<Self> {
        // Extract only integer literals
        let mut literals = BTreeSet::new();
        for arm in arms {
            if let PatternValue::IntLiteral(val) = arm.pattern {
                literals.insert(val);
            }
        }

        if literals.is_empty() {
            return None;
        }

        let min = *literals.first()?;
        let max = *literals.last()?;
        let range = (max - min + 1) as usize;

        // Check density: if sparse, don't create table
        let density = literals.len() as f64 / range as f64;
        if density < 0.3 {
            return None;
        }

        let mut entries = vec![None; range];

        for arm in arms {
            if let PatternValue::IntLiteral(val) = arm.pattern {
                let index = (val - min) as usize;
                entries[index] = Some(arm.action_id);
            }
        }

        Some(JumpTable {
            base: min,
            size: range,
            entries,
            default_action,
        })
    }

    /// Look up an action by value.
    pub fn lookup(&self, value: i64) -> u32 {
        let index = (value - self.base) as usize;
        if index < self.entries.len() {
            self.entries[index].unwrap_or(self.default_action)
        } else {
            self.default_action
        }
    }

    /// Compute metrics for this table.
    pub fn compute_metrics(&self) -> JumpTableMetrics {
        let pattern_count = self.entries.iter().filter(|e| e.is_some()).count();
        let value_range = self.size;
        let density = pattern_count as f64 / value_range as f64;

        // Jump table: base + size * 8 bytes (pointer per entry)
        let jump_table_bytes = 8 + (self.size * 8);

        // Cascading if/else: 12 bytes per comparison + branch
        let cascade_bytes = pattern_count * 12;

        JumpTableMetrics {
            pattern_count,
            value_range,
            density,
            jump_table_bytes,
            cascade_bytes,
        }
    }
}

/// Decision to use jump table or cascade comparisons.
#[derive(Clone, Debug)]
pub enum MatchLowering {
    JumpTable(JumpTable),
    CascadeComparisons {
        arms: Vec<(PatternValue, u32)>,
        default_action: u32,
    },
}

/// Analyze a match expression and choose the best lowering strategy.
pub fn analyze_match(arms: &[MatchArm], default_action: u32) -> MatchLowering {
    // Try to create a jump table
    if let Some(table) = JumpTable::from_arms(arms, default_action) {
        let metrics = table.compute_metrics();
        if metrics.should_use_jump_table() {
            return MatchLowering::JumpTable(table);
        }
    }

    // Fall back to cascading comparisons
    let arm_pairs = arms
        .iter()
        .map(|arm| (arm.pattern.clone(), arm.action_id))
        .collect();

    MatchLowering::CascadeComparisons {
        arms: arm_pairs,
        default_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jump_table_creation() {
        let arms = vec![
            MatchArm {
                pattern: PatternValue::IntLiteral(1),
                action_id: 10,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(2),
                action_id: 20,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(5),
                action_id: 50,
            },
        ];

        let table = JumpTable::from_arms(&arms, 0).unwrap();
        assert_eq!(table.base, 1);
        assert_eq!(table.size, 5);

        assert_eq!(table.lookup(1), 10);
        assert_eq!(table.lookup(2), 20);
        assert_eq!(table.lookup(3), 0); // default
        assert_eq!(table.lookup(5), 50);
    }

    #[test]
    fn test_jump_table_metrics() {
        let arms = vec![
            MatchArm {
                pattern: PatternValue::IntLiteral(0),
                action_id: 0,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(1),
                action_id: 1,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(2),
                action_id: 2,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(3),
                action_id: 3,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(4),
                action_id: 4,
            },
        ];

        let table = JumpTable::from_arms(&arms, 99).unwrap();
        let metrics = table.compute_metrics();

        assert_eq!(metrics.pattern_count, 5);
        assert_eq!(metrics.value_range, 5);
        assert_eq!(metrics.density, 1.0);
        assert!(metrics.should_use_jump_table());
    }

    #[test]
    fn test_sparse_patterns_no_table() {
        let arms = vec![
            MatchArm {
                pattern: PatternValue::IntLiteral(0),
                action_id: 0,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(1000),
                action_id: 1,
            },
        ];

        // Sparse pattern (density 0.002) should not create a table
        let table = JumpTable::from_arms(&arms, 99);
        assert!(table.is_none());
    }

    #[test]
    fn test_match_lowering_strategy() {
        let dense_arms = vec![
            MatchArm {
                pattern: PatternValue::IntLiteral(1),
                action_id: 10,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(2),
                action_id: 20,
            },
            MatchArm {
                pattern: PatternValue::IntLiteral(3),
                action_id: 30,
            },
        ];

        let lowering = analyze_match(&dense_arms, 0);
        match lowering {
            MatchLowering::JumpTable(_) => {
                // Dense pattern should use jump table
            }
            MatchLowering::CascadeComparisons { .. } => {
                panic!("Expected jump table for dense pattern");
            }
        }
    }
}
