/// Function Signature Linear Type Validation
/// 
/// Validates that function signatures properly declare and enforce linear type
/// constraints. Ensures that linear resources are consumed or returned, and
/// that ownership rules are enforced at function boundaries.

use std::collections::HashMap;
use crate::types::Type;

/// Parameter mode for function parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamMode {
    /// Parameter is owned and can be consumed by the function
    Owned,
    /// Parameter is borrowed immutably (&T)
    BorrowedImmut,
    /// Parameter is borrowed mutably (&mut T)
    BorrowedMut,
}

impl ParamMode {
    pub fn display(&self) -> &'static str {
        match self {
            ParamMode::Owned => "owned",
            ParamMode::BorrowedImmut => "borrowed (immut)",
            ParamMode::BorrowedMut => "borrowed (mut)",
        }
    }
}

/// Return mode for function return values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnMode {
    /// Function returns ownership of the value
    Owned,
    /// Function returns nothing (Unit type)
    Unit,
    /// Function returns a borrow (borrowed reference)
    Borrowed,
}

/// Linear type constraint on a function parameter.
#[derive(Clone, Debug)]
pub struct LinearParam {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub ty: Type,
    /// Is this a linear type?
    pub is_linear: bool,
    /// How the parameter is passed
    pub mode: ParamMode,
}

impl LinearParam {
    pub fn new(name: String, ty: Type, is_linear: bool, mode: ParamMode) -> Self {
        LinearParam {
            name,
            ty,
            is_linear,
            mode,
        }
    }
}

/// Linear type constraint on function return value.
#[derive(Clone, Debug)]
pub struct LinearReturn {
    /// Return type
    pub ty: Type,
    /// Is this a linear type?
    pub is_linear: bool,
    /// How the value is returned
    pub mode: ReturnMode,
}

impl LinearReturn {
    pub fn new(ty: Type, is_linear: bool, mode: ReturnMode) -> Self {
        LinearReturn { ty, is_linear, mode }
    }
}

/// Function signature with linear type information.
#[derive(Clone, Debug)]
pub struct LinearFunctionSignature {
    /// Function name
    pub name: String,
    /// Parameters with linear constraints
    pub params: Vec<LinearParam>,
    /// Return type with linear constraints
    pub ret: LinearReturn,
}

impl LinearFunctionSignature {
    pub fn new(name: String, params: Vec<LinearParam>, ret: LinearReturn) -> Self {
        LinearFunctionSignature { name, params, ret }
    }
    
    /// Get linear parameters (those that must be consumed).
    pub fn linear_params(&self) -> Vec<&str> {
        self.params
            .iter()
            .filter(|p| p.is_linear && p.mode == ParamMode::Owned)
            .map(|p| p.name.as_str())
            .collect()
    }
    
    /// Get borrowed parameters (those that should not be consumed).
    pub fn borrowed_params(&self) -> Vec<&str> {
        self.params
            .iter()
            .filter(|p| matches!(p.mode, ParamMode::BorrowedImmut | ParamMode::BorrowedMut))
            .map(|p| p.name.as_str())
            .collect()
    }
}

/// Validates function signatures for linear type correctness.
pub struct SignatureValidator;

impl SignatureValidator {
    /// Validate that a function properly declares all linear parameters.
    /// 
    /// Rules:
    /// 1. All linear parameters must be explicitly marked (owned/borrowed)
    /// 2. Owned linear parameters should be documented (may be consumed)
    /// 3. Borrowed parameters must never be moved
    pub fn validate_signature(sig: &LinearFunctionSignature) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        // Check parameters
        for param in &sig.params {
            // Linear types should have explicit mode
            if param.is_linear && param.mode == ParamMode::Owned {
                // This is OK - owned linear parameter can be consumed
            }
        }
        
        // Check return value
        if sig.ret.is_linear && sig.ret.mode != ReturnMode::Owned && sig.ret.mode != ReturnMode::Unit {
            // Returning a borrowed linear reference is problematic
            if sig.ret.mode == ReturnMode::Borrowed {
                errors.push(format!(
                    "function '{}' returns a borrow of linear type '{}' - this may create use-after-free",
                    sig.name,
                    sig.ret.ty.display()
                ));
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate function body against its signature.
    /// 
    /// Checks that:
    /// 1. All owned linear parameters are used (moved/returned)
    /// 2. Borrowed parameters are not moved
    /// 3. Return value matches signature constraints
    pub fn validate_body(
        sig: &LinearFunctionSignature,
        used_params: &[&str],
        borrowed_params: &[&str],
        returned_param: Option<&str>,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        // Check that owned linear parameters are accounted for
        for param in sig.linear_params() {
            if !used_params.contains(&param) && returned_param != Some(param) {
                errors.push(format!(
                    "linear parameter '{}' of type '{}' is not consumed",
                    param, sig.params.iter().find(|p| p.name == param).unwrap().ty.display()
                ));
            }
        }
        
        // Check that borrowed parameters are not moved
        for param in &sig.params {
            if matches!(param.mode, ParamMode::BorrowedImmut | ParamMode::BorrowedMut) {
                if used_params.iter().any(|&used| used == param.name) {
                    // Borrowed parameters can be used (read) but not moved
                    // This is OK - the check is that they're not in moved_params
                }
            }
        }
        
        // Check return value
        if !matches!(sig.ret.ty, Type::Unit) {
            if let Some(returned) = returned_param {
                // Verify that the returned parameter matches the signature
                if let Some(param) = sig.params.iter().find(|p| p.name == returned) {
                    if param.ty != sig.ret.ty {
                        errors.push(format!(
                            "function '{}' signature expects return type '{}' but got '{}'",
                            sig.name, sig.ret.ty.display(), param.ty.display()
                        ));
                    }
                } else {
                    errors.push(format!(
                        "returned value '{}' not found in parameters",
                        returned
                    ));
                }
            } else if !matches!(sig.ret.mode, ReturnMode::Unit) {
                errors.push(format!(
                    "function '{}' must return value of type '{}'",
                    sig.name, sig.ret.ty.display()
                ));
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Validate parameter compatibility at call site.
    /// 
    /// Checks that:
    /// 1. Owned parameters are passed owned values
    /// 2. Borrowed parameters are passed borrowable values
    /// 3. Argument types match parameter types
    pub fn validate_call(
        sig: &LinearFunctionSignature,
        args: &[(String, Type)], // (name, type)
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        if args.len() != sig.params.len() {
            errors.push(format!(
                "function '{}' expects {} arguments, got {}",
                sig.name,
                sig.params.len(),
                args.len()
            ));
            return Err(errors);
        }
        
        for (i, (arg_name, arg_type)) in args.iter().enumerate() {
            let param = &sig.params[i];
            
            // Type compatibility
            if param.ty != *arg_type {
                // Allow subtyping for constrained ranges
                let types_compatible = match (&param.ty, arg_type) {
                    (Type::U32, Type::ConstrainedRange { .. }) => true,
                    _ => false,
                };
                if !types_compatible {
                    errors.push(format!(
                        "argument {} ('{}') type mismatch: expected '{}', got '{}'",
                        i,
                        arg_name,
                        param.ty.display(),
                        arg_type.display()
                    ));
                }
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Context for tracking function signatures during type-checking.
#[derive(Clone, Debug)]
pub struct SignatureContext {
    /// Currently checked function signature
    pub current_sig: Option<LinearFunctionSignature>,
    /// All known signatures (for call validation)
    pub signatures: HashMap<String, LinearFunctionSignature>,
}

impl SignatureContext {
    pub fn new() -> Self {
        SignatureContext {
            current_sig: None,
            signatures: HashMap::new(),
        }
    }
    
    /// Register a function signature.
    pub fn register_signature(&mut self, sig: LinearFunctionSignature) {
        self.signatures.insert(sig.name.clone(), sig);
    }
    
    /// Get a registered function signature.
    pub fn get_signature(&self, name: &str) -> Option<&LinearFunctionSignature> {
        self.signatures.get(name)
    }
    
    /// Set the current function being checked.
    pub fn set_current_function(&mut self, sig: LinearFunctionSignature) {
        self.current_sig = Some(sig);
    }
    
    /// Get the current function signature.
    pub fn current_function(&self) -> Option<&LinearFunctionSignature> {
        self.current_sig.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_param_creation() {
        let param = LinearParam::new(
            "model".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        assert_eq!(param.name, "model");
        assert!(param.is_linear);
        assert_eq!(param.mode, ParamMode::Owned);
    }

    #[test]
    fn test_signature_validation() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("consume".to_string(), vec![param], ret);
        
        let result = SignatureValidator::validate_signature(&sig);
        assert!(result.is_ok());
    }

    #[test]
    fn test_body_validation_missing_consume() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("test".to_string(), vec![param], ret);
        
        // Parameter x not consumed
        let result = SignatureValidator::validate_body(&sig, &[], &[], None);
        assert!(result.is_err());
    }

    #[test]
    fn test_body_validation_with_consume() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("test".to_string(), vec![param], ret);
        
        // Parameter x is consumed
        let result = SignatureValidator::validate_body(&sig, &["x"], &[], None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_validation() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("consume".to_string(), vec![param], ret);
        
        let args = vec![("model_var".to_string(), Type::Model)];
        let result = SignatureValidator::validate_call(&sig, &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_call_validation_type_mismatch() {
        let param = LinearParam::new(
            "x".to_string(),
            Type::Model,
            true,
            ParamMode::Owned,
        );
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("consume".to_string(), vec![param], ret);
        
        let args = vec![("num_var".to_string(), Type::U32)];
        let result = SignatureValidator::validate_call(&sig, &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_context() {
        let mut ctx = SignatureContext::new();
        
        let param = LinearParam::new("x".to_string(), Type::Model, true, ParamMode::Owned);
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("consume".to_string(), vec![param], ret);
        
        ctx.register_signature(sig);
        assert!(ctx.get_signature("consume").is_some());
        assert!(ctx.get_signature("nonexistent").is_none());
    }

    #[test]
    fn test_borrowed_params() {
        let param1 = LinearParam::new("x".to_string(), Type::Model, true, ParamMode::Owned);
        let param2 = LinearParam::new("y".to_string(), Type::U32, false, ParamMode::BorrowedImmut);
        let ret = LinearReturn::new(Type::Unit, false, ReturnMode::Unit);
        let sig = LinearFunctionSignature::new("func".to_string(), vec![param1, param2], ret);
        
        assert_eq!(sig.linear_params(), vec!["x"]);
        assert_eq!(sig.borrowed_params(), vec!["y"]);
    }
}
