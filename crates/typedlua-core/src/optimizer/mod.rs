use crate::config::OptimizationLevel;
use crate::diagnostics::DiagnosticHandler;
use crate::errors::CompilationError;
use std::sync::Arc;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringInterner;

mod passes;
use passes::*;

mod rich_enum_optimization;
use rich_enum_optimization::*;

mod method_to_function_conversion;
use method_to_function_conversion::*;

mod devirtualization;
use devirtualization::DevirtualizationPass;

mod operator_inlining;
use operator_inlining::OperatorInliningPass;

mod interface_inlining;
use interface_inlining::InterfaceMethodInliningPass;

mod aggressive_inlining;
use aggressive_inlining::AggressiveInliningPass;

/// Trait for optimization passes that transform the AST
pub trait OptimizationPass {
    /// Get the name of this optimization pass
    fn name(&self) -> &'static str;

    /// Run this optimization pass on the program
    /// Returns true if the pass made changes to the AST
    fn run(&mut self, program: &mut Program) -> Result<bool, CompilationError>;

    /// Get the minimum optimization level required for this pass
    fn min_level(&self) -> OptimizationLevel;
}

/// Optimizer for AST transformations
pub struct Optimizer {
    level: OptimizationLevel,
    #[allow(dead_code)]
    handler: Arc<dyn DiagnosticHandler>,
    interner: Arc<StringInterner>,
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl Optimizer {
    /// Create a new optimizer with the given optimization level
    pub fn new(
        level: OptimizationLevel,
        handler: Arc<dyn DiagnosticHandler>,
        interner: Arc<StringInterner>,
    ) -> Self {
        let mut optimizer = Self {
            level,
            handler,
            interner,
            passes: Vec::new(),
        };

        // Register optimization passes based on level
        optimizer.register_passes();
        optimizer
    }

    /// Register optimization passes based on the optimization level
    fn register_passes(&mut self) {
        let interner = self.interner.clone();

        // O1 passes - Basic optimizations (5 passes)
        self.passes.push(Box::new(ConstantFoldingPass));
        self.passes.push(Box::new(DeadCodeEliminationPass));
        self.passes.push(Box::new(AlgebraicSimplificationPass));
        self.passes.push(Box::new(TablePreallocationPass));
        self.passes
            .push(Box::new(GlobalLocalizationPass::new(interner.clone())));

        // O2 passes - Standard optimizations (7 passes)
        let mut inlining_pass = FunctionInliningPass::default();
        inlining_pass.set_interner(interner.clone());
        self.passes.push(Box::new(inlining_pass));
        self.passes.push(Box::new(LoopOptimizationPass));
        let mut concat_pass = StringConcatOptimizationPass::default();
        concat_pass.set_interner(interner.clone());
        self.passes.push(Box::new(concat_pass));
        self.passes.push(Box::new(DeadStoreEliminationPass));
        self.passes.push(Box::new(TailCallOptimizationPass));
        self.passes.push(Box::new(RichEnumOptimizationPass));
        self.passes
            .push(Box::new(MethodToFunctionConversionPass::new(
                interner.clone(),
            )));

        // O3 passes - Aggressive optimizations (6 passes)
        self.passes
            .push(Box::new(AggressiveInliningPass::new(interner.clone())));
        self.passes
            .push(Box::new(OperatorInliningPass::new(interner.clone())));
        self.passes
            .push(Box::new(InterfaceMethodInliningPass::new(interner.clone())));
        self.passes
            .push(Box::new(DevirtualizationPass::new(interner.clone())));
        let mut generic_spec_pass = GenericSpecializationPass::default();
        generic_spec_pass.set_interner(interner.clone());
        self.passes.push(Box::new(generic_spec_pass));
    }

    /// Returns the number of registered passes
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Returns the names of all registered passes
    pub fn pass_names(&self) -> Vec<&'static str> {
        self.passes.iter().map(|p| p.name()).collect()
    }

    /// Optimize the program AST
    /// Runs all registered optimization passes until no more changes are made
    pub fn optimize(&mut self, program: &mut Program) -> Result<(), CompilationError> {
        // Resolve Auto to actual optimization level based on build profile
        let effective_level = self.level.effective();

        if effective_level == OptimizationLevel::O0 {
            // No optimizations at O0
            return Ok(());
        }

        // Run passes in a loop until no changes are made (fixed-point iteration)
        let mut iteration = 0;
        let max_iterations = 10; // Prevent infinite loops

        loop {
            let mut changed = false;
            iteration += 1;

            if iteration > max_iterations {
                // Safety limit reached - stop optimizing
                break;
            }

            // Run all passes that are eligible for the current optimization level
            for pass in &mut self.passes {
                // Only run passes that are at or below the current optimization level
                if pass.min_level() <= effective_level {
                    let pass_changed = pass.run(program)?;
                    changed |= pass_changed;
                }
            }

            // If no pass made changes, we've reached a fixed point
            if !changed {
                break;
            }
        }

        Ok(())
    }
}
