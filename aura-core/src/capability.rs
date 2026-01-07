#![forbid(unsafe_code)]

use std::collections::HashMap;

use aura_ast::Span;
use aura_ir::CapabilityId;

use crate::error::SemanticError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityKind {
    Root,
    Read,
    Write,
    Move,
}

#[derive(Clone, Debug)]
pub struct CapabilityEdge {
    pub from: CapabilityId,
    pub to: CapabilityId,
    pub kind: CapabilityKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct CapabilityNode {
    pub id: CapabilityId,
    pub name: String,
    pub alive: bool,
    pub consumed_at: Option<Span>,
}

#[derive(Default, Debug)]
pub struct CapabilityGraph {
    pub nodes: HashMap<CapabilityId, CapabilityNode>,
    pub edges: Vec<CapabilityEdge>,
    pub by_value: HashMap<String, CapabilityId>,
}

impl CapabilityGraph {
    pub fn alloc_root(
        &mut self,
        id: CapabilityId,
        value_name: &str,
    ) {
        self.nodes.insert(
            id,
            CapabilityNode {
                id,
                name: value_name.to_string(),
                alive: true,
                consumed_at: None,
            },
        );
        self.by_value.insert(value_name.to_string(), id);
    }

    pub fn ensure_alive(&self, value_name: &str, span: Span) -> Result<CapabilityId, SemanticError> {
        let id = self.by_value.get(value_name).copied().ok_or_else(|| SemanticError {
            message: format!("unknown value '{value_name}'"),
            span,
        })?;
        let node = self.nodes.get(&id).expect("cap node");
        if !node.alive {
            let extra = node
                .consumed_at
                .map(|s| format!(
                    " (consumed at byte offset {} len {})",
                    s.offset(),
                    s.len()
                ))
                .unwrap_or_default();
            return Err(SemanticError {
                message: format!("use after move: '{value_name}'{extra}"),
                span,
            });
        }
        Ok(id)
    }

    pub fn lend_read(&mut self, from: CapabilityId, to: CapabilityId, span: Span) {
        self.edges.push(CapabilityEdge {
            from,
            to,
            kind: CapabilityKind::Read,
            span,
        });
    }

    pub fn lend_write(&mut self, from: CapabilityId, to: CapabilityId, span: Span) {
        self.edges.push(CapabilityEdge {
            from,
            to,
            kind: CapabilityKind::Write,
            span,
        });
    }

    pub fn consume_move(&mut self, from: CapabilityId, to: CapabilityId, span: Span) {
        self.edges.push(CapabilityEdge {
            from,
            to,
            kind: CapabilityKind::Move,
            span,
        });
        if let Some(node) = self.nodes.get_mut(&from) {
            node.alive = false;
            node.consumed_at = Some(span);
        }
    }
}
