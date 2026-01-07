// Counterexample Mapper: Z3 Model → Aura AST Types
//
// Maps SMT solver models (from Z3) into typed Aura values that preserve source-level semantics.
// Supports primitives, records, enums, arrays, and nested structures.
//
// Design principles:
// 1. Deterministic: same Z3 model produces same TypedValue across runs
// 2. Failure-tolerant: fallback to string if Z3 model missing or unparseable
// 3. Type-driven: leverages Aura's type system for accurate mapping
// 4. Minimal: only include values needed to explain failure

use std::collections::BTreeMap;
use std::fmt;

/// A typed value extracted from a Z3 model, preserving Aura semantic structure.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedValue {
    /// Primitive: integer, boolean, or byte
    Primitive {
        typ: String,           // "u32", "bool", "i64", etc.
        value: String,         // "42", "true", "0xFF", etc.
    },
    /// Record: struct with named fields
    Record {
        name: String,          // "Point", "Config", etc.
        fields: BTreeMap<String, TypedValue>,
    },
    /// Enum: variant with optional payload
    Enum {
        name: String,          // "Option", "Result", etc.
        variant: String,       // "Some", "Ok", "Err", etc.
        payload: Option<Box<TypedValue>>,
    },
    /// Array: homogeneous sequence
    Array {
        element_type: String,  // "u32", "bool", etc.
        elements: Vec<TypedValue>,
    },
    /// Reference/pointer
    Reference {
        referent: Box<TypedValue>,
        mutable: bool,
    },
    /// Tuple: positional fields
    Tuple(Vec<TypedValue>),
    /// Function reference (opaque)
    Function {
        name: String,
        arity: usize,
    },
    /// Unknown or fallback value
    Unknown {
        reason: String,
        fallback_json: Option<String>,
    },
}

impl TypedValue {
    /// Pretty-print for user-facing display (Explain panel)
    pub fn display(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        match self {
            TypedValue::Primitive { typ, value } => {
                format!("{} : {} = {}", pad, typ, value)
            }
            TypedValue::Record { name, fields } => {
                let mut result = format!("{}{} {{\n", pad, name);
                for (fname, fval) in fields {
                    result.push_str(&format!("{}  {} : {},\n", pad, fname, fval.display_compact()));
                }
                result.push_str(&format!("{}}}", pad));
                result
            }
            TypedValue::Enum { name, variant, payload } => {
                if let Some(p) = payload {
                    format!("{}{}::{} ({})", pad, name, variant, p.display_compact())
                } else {
                    format!("{}{}::{}", pad, name, variant)
                }
            }
            TypedValue::Array { element_type, elements } => {
                if elements.is_empty() {
                    format!("{}[{}; 0]", pad, element_type)
                } else if elements.len() <= 5 {
                    let vals = elements.iter()
                        .map(|e| e.display_compact())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}[{}; {}]", pad, element_type, vals)
                } else {
                    format!("{}[{}; {} elements]", pad, element_type, elements.len())
                }
            }
            TypedValue::Reference { referent, mutable } => {
                let mut_kw = if *mutable { "mut " } else { "" };
                format!("{}&{}{}", pad, mut_kw, referent.display_compact())
            }
            TypedValue::Tuple(elems) => {
                let vals = elems.iter()
                    .map(|e| e.display_compact())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", pad, vals)
            }
            TypedValue::Function { name, arity } => {
                format!("{}fn {} / {}", pad, name, arity)
            }
            TypedValue::Unknown { reason, fallback_json } => {
                if let Some(json) = fallback_json {
                    format!("{}<unknown: {}: {}>", pad, reason, json)
                } else {
                    format!("{}<unknown: {}>", pad, reason)
                }
            }
        }
    }

    /// Compact single-line representation
    pub fn display_compact(&self) -> String {
        match self {
            TypedValue::Primitive { value, .. } => value.clone(),
            TypedValue::Record { name, fields: _ } => format!("{}{{...}}", name),
            TypedValue::Enum { variant, payload: None, .. } => variant.clone(),
            TypedValue::Enum { variant, payload: Some(p), .. } => {
                format!("{}({})", variant, p.display_compact())
            }
            TypedValue::Array { elements, .. } => {
                if elements.is_empty() {
                    "[]".to_string()
                } else {
                    format!("[...{}]", elements.len())
                }
            }
            TypedValue::Reference { referent, mutable } => {
                let mut_kw = if *mutable { "mut " } else { "" };
                format!("&{}{}", mut_kw, referent.display_compact())
            }
            TypedValue::Tuple(elems) => {
                let vals = elems.iter()
                    .map(|e| e.display_compact())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", vals)
            }
            TypedValue::Function { name, .. } => name.clone(),
            TypedValue::Unknown { reason, .. } => format!("<{}>", reason),
        }
    }

    /// Check if value represents a "true" or failure-inducing state
    pub fn is_truthy(&self) -> bool {
        match self {
            TypedValue::Primitive { value, .. } => {
                match value.as_str() {
                    "true" | "1" | "True" => true,
                    "false" | "0" | "False" => false,
                    _ => !value.is_empty(),
                }
            }
            TypedValue::Enum { variant, .. } => {
                variant != "None" && variant != "false"
            }
            TypedValue::Unknown { .. } => false,
            _ => true,
        }
    }
}

impl fmt::Display for TypedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display(0))
    }
}

/// Maps Z3 model entries to Aura TypedValues
pub struct CounterexampleMapper {
    /// Type context: variable name → type signature
    type_context: BTreeMap<String, String>,
    /// Custom type definitions (struct name → field types)
    type_defs: BTreeMap<String, Vec<(String, String)>>,
}

impl CounterexampleMapper {
    /// Create a new mapper with type context
    pub fn new(type_context: BTreeMap<String, String>) -> Self {
        CounterexampleMapper {
            type_context,
            type_defs: BTreeMap::new(),
        }
    }

    /// Register a struct type definition
    pub fn register_struct(
        &mut self,
        name: String,
        fields: Vec<(String, String)>,
    ) {
        self.type_defs.insert(name, fields);
    }

    /// Register an enum type definition
    pub fn register_enum(
        &mut self,
        _name: String,
        _variants: Vec<String>,
    ) {
        // For now, enums are handled dynamically during mapping
        // Full support can expand this later
    }

    /// Map a Z3 model value to a TypedValue
    /// Expected Z3 format: string representation of value
    pub fn map_value(&self, name: &str, z3_model: &str) -> TypedValue {
        let typ = self.type_context.get(name)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        self.map_value_typed(&typ, z3_model)
    }

    /// Internal recursive mapping using type information
    fn map_value_typed(&self, typ: &str, value: &str) -> TypedValue {
        // Determine if this is a primitive, collection, or composite
        if self.is_primitive_type(typ) {
            self.map_primitive(typ, value)
        } else if typ.contains("[]") || typ.starts_with("Vec<") {
            self.map_array(typ, value)
        } else if typ.contains("(") && typ.contains(")") {
            self.map_tuple(value)
        } else if let Some(fields) = self.type_defs.get(typ) {
            self.map_record(typ.to_string(), fields.clone(), value)
        } else if typ.contains("::") {
            // Likely an enum (e.g., "Option::Some" or "Result::Ok")
            self.map_enum(typ, value)
        } else {
            TypedValue::Unknown {
                reason: format!("unhandled type: {}", typ),
                fallback_json: Some(value.to_string()),
            }
        }
    }

    /// Map a primitive value (u32, bool, i64, etc.)
    fn map_primitive(&self, typ: &str, value: &str) -> TypedValue {
        TypedValue::Primitive {
            typ: typ.to_string(),
            value: value.to_string(),
        }
    }

    /// Map an array/vector
    fn map_array(&self, typ: &str, value: &str) -> TypedValue {
        // Extract element type (e.g., "u32" from "Vec<u32>" or "u32[]")
        let element_type = self.extract_element_type(typ);

        // Very basic parsing: split by comma (not robust, but safe)
        let elements = if value.starts_with('[') && value.ends_with(']') {
            let inner = &value[1..value.len() - 1];
            if inner.is_empty() {
                vec![]
            } else {
                inner.split(',')
                    .map(|s| self.map_value_typed(&element_type, s.trim()))
                    .collect()
            }
        } else {
            vec![]
        };

        TypedValue::Array {
            element_type,
            elements,
        }
    }

    /// Map a tuple
    fn map_tuple(&self, value: &str) -> TypedValue {
        let elems = if value.starts_with('(') && value.ends_with(')') {
            let inner = &value[1..value.len() - 1];
            if inner.is_empty() {
                vec![]
            } else {
                inner.split(',')
                    .map(|s| self.map_value_typed("unknown", s.trim()))
                    .collect()
            }
        } else {
            vec![]
        };

        TypedValue::Tuple(elems)
    }

    /// Map a record/struct
    fn map_record(
        &self,
        name: String,
        fields: Vec<(String, String)>,
        value: &str,
    ) -> TypedValue {
        let mut mapped_fields = BTreeMap::new();

        // Very basic parsing: expect "{field1: val1, field2: val2, ...}"
        // This is a fallback; in real usage, Z3 would provide structured data
        if value.starts_with('{') && value.ends_with('}') {
            let inner = &value[1..value.len() - 1];
            for (field_name, field_type) in fields {
                // Try to find "field_name: <value>"
                if let Some(pos) = inner.find(&format!("{}:", field_name)) {
                    let start = pos + field_name.len() + 1;
                    let rest = &inner[start..];
                    let val = rest.split(',').next().unwrap_or("").trim();
                    mapped_fields.insert(
                        field_name,
                        self.map_value_typed(&field_type, val),
                    );
                }
            }
        }

        TypedValue::Record {
            name,
            fields: mapped_fields,
        }
    }

    /// Map an enum variant
    fn map_enum(&self, typ: &str, value: &str) -> TypedValue {
        // Heuristic: parse "Variant" or "Variant(payload)"
        if value.contains('(') {
            if let Some(paren_pos) = value.find('(') {
                let variant = value[..paren_pos].trim().to_string();
                let payload_str = &value[paren_pos + 1..value.len() - 1].trim();
                let payload = Box::new(self.map_value_typed("unknown", payload_str));
                TypedValue::Enum {
                    name: typ.to_string(),
                    variant,
                    payload: Some(payload),
                }
            } else {
                TypedValue::Unknown {
                    reason: format!("malformed enum: {}", typ),
                    fallback_json: Some(value.to_string()),
                }
            }
        } else {
            TypedValue::Enum {
                name: typ.to_string(),
                variant: value.to_string(),
                payload: None,
            }
        }
    }

    /// Check if a type string is a primitive
    fn is_primitive_type(&self, typ: &str) -> bool {
        matches!(typ,
            "bool" | "u8" | "u16" | "u32" | "u64" | "u128"
            | "i8" | "i16" | "i32" | "i64" | "i128"
            | "f32" | "f64" | "char" | "str"
        )
    }

    /// Extract element type from container (e.g., "u32" from "Vec<u32>")
    fn extract_element_type(&self, container_type: &str) -> String {
        if container_type.ends_with("[]") {
            container_type[..container_type.len() - 2].to_string()
        } else if let Some(start) = container_type.find('<') {
            if let Some(end) = container_type.find('>') {
                container_type[start + 1..end].to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_primitive_u32() {
        let mapper = CounterexampleMapper::new(BTreeMap::new());
        let z3_val = "42";
        
        let result = mapper.map_primitive("u32", z3_val);
        
        assert_eq!(
            result,
            TypedValue::Primitive {
                typ: "u32".to_string(),
                value: "42".to_string(),
            }
        );
    }

    #[test]
    fn test_map_primitive_bool() {
        let mapper = CounterexampleMapper::new(BTreeMap::new());
        let z3_val = "true";
        
        let result = mapper.map_primitive("bool", z3_val);
        
        assert_eq!(
            result,
            TypedValue::Primitive {
                typ: "bool".to_string(),
                value: "true".to_string(),
            }
        );
    }

    #[test]
    fn test_map_array() {
        let mapper = CounterexampleMapper::new(BTreeMap::new());
        let z3_val = "[1, 2, 3]";
        
        let result = mapper.map_array("Vec<u32>", z3_val);
        
        if let TypedValue::Array { element_type, elements } = result {
            assert_eq!(element_type, "u32");
            assert_eq!(elements.len(), 3);
        } else {
            panic!("expected TypedValue::Array");
        }
    }

    #[test]
    fn test_map_record() {
        let mut mapper = CounterexampleMapper::new(BTreeMap::new());
        mapper.register_struct(
            "Point".to_string(),
            vec![
                ("x".to_string(), "u32".to_string()),
                ("y".to_string(), "u32".to_string()),
            ],
        );

        let z3_val = "{x: 10, y: 20}";

        let result = mapper.map_record(
            "Point".to_string(),
            vec![
                ("x".to_string(), "u32".to_string()),
                ("y".to_string(), "u32".to_string()),
            ],
            z3_val,
        );

        if let TypedValue::Record { name, fields } = result {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
        } else {
            panic!("expected TypedValue::Record");
        }
    }

    #[test]
    fn test_map_enum() {
        let mapper = CounterexampleMapper::new(BTreeMap::new());
        let z3_val = "Some(42)";

        let result = mapper.map_enum("Option", z3_val);

        if let TypedValue::Enum { variant, payload, .. } = result {
            assert_eq!(variant, "Some");
            assert!(payload.is_some());
        } else {
            panic!("expected TypedValue::Enum");
        }
    }

    #[test]
    fn test_typed_value_display() {
        let prim = TypedValue::Primitive {
            typ: "u32".to_string(),
            value: "42".to_string(),
        };
        
        let compact = prim.display_compact();
        assert_eq!(compact, "42");
        
        let full = prim.display(0);
        assert!(full.contains("u32"));
        assert!(full.contains("42"));
    }

    #[test]
    fn test_typed_value_is_truthy() {
        let true_val = TypedValue::Primitive {
            typ: "bool".to_string(),
            value: "true".to_string(),
        };
        assert!(true_val.is_truthy());

        let false_val = TypedValue::Primitive {
            typ: "bool".to_string(),
            value: "false".to_string(),
        };
        assert!(!false_val.is_truthy());
    }
}
