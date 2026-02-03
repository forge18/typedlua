mod constant_folding;
pub use constant_folding::ConstantFoldingPass;

mod dead_code_elimination;
pub use dead_code_elimination::DeadCodeEliminationPass;

mod algebraic_simplification;
pub use algebraic_simplification::AlgebraicSimplificationPass;

mod table_preallocation;
pub use table_preallocation::TablePreallocationPass;

mod global_localization;
pub use global_localization::GlobalLocalizationPass;

mod function_inlining;
pub use function_inlining::FunctionInliningPass;

mod loop_optimization;
pub use loop_optimization::LoopOptimizationPass;

mod string_concat_optimization;
pub use string_concat_optimization::StringConcatOptimizationPass;

mod dead_store_elimination;
pub use dead_store_elimination::DeadStoreEliminationPass;

mod tail_call_optimization;
pub use tail_call_optimization::TailCallOptimizationPass;

mod generic_specialization;
pub use generic_specialization::GenericSpecializationPass;
