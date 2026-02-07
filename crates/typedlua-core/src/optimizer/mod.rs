//! AST optimizer for TypedLua
//!
//! NOTE: Optimization passes are temporarily disabled during arena allocation migration.
//! The optimizer's public API is preserved but `optimize()` is a no-op.
//! The pass infrastructure needs to be migrated to work with arena-allocated
//! immutable AST nodes (clone-and-replace pattern). See mod.rs.pre_arena for
//! the original implementation.

use crate::config::OptimizationLevel;
use crate::diagnostics::DiagnosticHandler;
use crate::MutableProgram;

use std::sync::Arc;
use tracing::info;
use typedlua_parser::string_interner::StringInterner;

// Keep devirtualization and whole_program_analysis modules since they provide
// public types used by codegen (ClassHierarchy, WholeProgramAnalysis)
mod devirtualization;
pub use devirtualization::ClassHierarchy;

mod whole_program_analysis;
pub use whole_program_analysis::WholeProgramAnalysis;

// =============================================================================
// Optimizer - Public API preserved, passes temporarily disabled
// =============================================================================

/// Optimizer for AST transformations
///
/// This struct manages optimization passes and runs them until a fixed point
/// is reached (no more changes). Passes are organized into composite groups
/// to minimize AST traversals.
///
/// NOTE: Optimization passes are temporarily disabled during arena allocation
/// migration. The `optimize()` method returns Ok(()) immediately.
pub struct Optimizer {
    level: OptimizationLevel,
    #[allow(dead_code)]
    handler: Arc<dyn DiagnosticHandler>,
    #[allow(dead_code)]
    interner: Arc<StringInterner>,

    // Whole-program analysis results (for O3+ cross-module optimizations)
    #[allow(dead_code)]
    whole_program_analysis: Option<WholeProgramAnalysis>,
}

impl Optimizer {
    /// Create a new optimizer with the given optimization level
    pub fn new(
        level: OptimizationLevel,
        handler: Arc<dyn DiagnosticHandler>,
        interner: Arc<StringInterner>,
    ) -> Self {
        Self {
            level,
            handler,
            interner,
            whole_program_analysis: None,
        }
    }

    /// Set whole-program analysis results for cross-module optimizations
    pub fn set_whole_program_analysis(&mut self, analysis: WholeProgramAnalysis) {
        self.whole_program_analysis = Some(analysis);
    }

    /// Returns the number of registered passes
    pub fn pass_count(&self) -> usize {
        0 // Passes temporarily disabled
    }

    /// Returns the names of all registered passes
    pub fn pass_names(&self) -> Vec<&'static str> {
        Vec::new() // Passes temporarily disabled
    }

    /// Optimize the program AST
    ///
    /// NOTE: Temporarily disabled during arena allocation migration.
    /// Returns Ok(()) immediately without running any passes.
    pub fn optimize(&mut self, _program: &mut MutableProgram<'_>) -> Result<(), String> {
        let effective_level = self.level.effective();

        if effective_level == OptimizationLevel::O0 {
            return Ok(());
        }

        // TODO: Re-enable optimization passes after migrating to arena-compatible
        // clone-and-replace pattern. See mod.rs.pre_arena for original implementation.
        info!(
            "Optimization passes temporarily disabled during arena migration (requested level: {:?})",
            effective_level
        );

        Ok(())
    }
}
