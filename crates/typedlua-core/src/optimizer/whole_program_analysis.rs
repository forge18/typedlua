//! Whole-program analysis infrastructure for parallel optimization
//!
//! This module provides analysis results that require cross-module information.
//! Analysis is built once sequentially, then shared (read-only) across parallel
//! optimization passes via Arc.

use crate::config::OptimizationLevel;
use crate::optimizer::devirtualization::ClassHierarchy;
use std::sync::Arc;
use typedlua_parser::ast::Program;

/// Thread-safe whole-program analysis results
///
/// This struct contains analysis that requires cross-module information.
/// It's built once sequentially before parallel code generation, then shared
/// (read-only) across parallel optimization passes.
#[derive(Clone, Debug)]
pub struct WholeProgramAnalysis {
    /// Class hierarchy for devirtualization
    pub class_hierarchy: Arc<ClassHierarchy>,
}

impl WholeProgramAnalysis {
    /// Build whole-program analysis by scanning all type-checked modules
    ///
    /// This should be called sequentially after type checking, before parallel
    /// code generation begins. The resulting analysis is thread-safe and can
    /// be cloned cheaply (Arc) for each parallel worker.
    pub fn build(programs: &[&Program], optimization_level: OptimizationLevel) -> Self {
        // Only build expensive analysis if O3+ optimization is enabled
        let class_hierarchy = if optimization_level >= OptimizationLevel::O3 {
            ClassHierarchy::build_multi_module(programs)
        } else {
            ClassHierarchy::default()
        };

        Self {
            class_hierarchy: Arc::new(class_hierarchy),
        }
    }
}
