//! Class hierarchy analysis for devirtualization
//!
//! Provides class hierarchy information used for cross-module optimizations.
//! The actual devirtualization pass is temporarily disabled during arena migration
//! (see devirtualization.rs.pre_arena for the full implementation).

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use tracing::debug;
use typedlua_parser::ast::expression::{Expression, ExpressionKind};
use typedlua_parser::ast::statement::{ClassMember, Statement};
use typedlua_parser::ast::types::TypeKind;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::StringId;

use ExpressionKind::*;

/// Class hierarchy information for devirtualization safety analysis
#[derive(Debug, Default, Clone)]
pub struct ClassHierarchy {
    /// class -> parent (None if no extends)
    parent_of: FxHashMap<StringId, Option<StringId>>,
    /// parent -> list of direct children
    children_of: FxHashMap<StringId, Vec<StringId>>,
    /// class -> is_final
    is_final: FxHashMap<StringId, bool>,
    /// (class, method) -> is_final
    final_methods: FxHashMap<(StringId, StringId), bool>,
    /// (class, method) -> method is declared here (not inherited)
    declares_method: FxHashMap<(StringId, StringId), bool>,
    /// Set of all known class names (to distinguish from interfaces)
    known_classes: FxHashMap<StringId, bool>,
    /// RTA: class -> set of subclasses that are instantiated
    instantiated_subclasses: FxHashMap<StringId, FxHashSet<StringId>>,
    /// RTA: For each class, the single instantiated subclass if there's exactly one
    single_instantiated_subclass: FxHashMap<StringId, StringId>,
    /// RTA: Total count of instantiations per class
    instantiation_counts: FxHashMap<StringId, usize>,
    /// RTA: Set of all classes that have any instantiations
    classes_with_instantiations: FxHashSet<StringId>,
}

impl ClassHierarchy {
    /// Build class hierarchy by scanning all class declarations in the program
    pub fn build<'arena>(program: &Program<'arena>) -> Self {
        let mut hierarchy = ClassHierarchy::default();

        for stmt in program.statements.iter() {
            if let Statement::Class(class) = stmt {
                let class_id = class.name.node;
                hierarchy.known_classes.insert(class_id, true);
                hierarchy.is_final.insert(class_id, class.is_final);

                let parent_id = class.extends.as_ref().and_then(|ext| {
                    if let TypeKind::Reference(type_ref) = &ext.kind {
                        Some(type_ref.name.node)
                    } else {
                        None
                    }
                });
                hierarchy.parent_of.insert(class_id, parent_id);

                if let Some(parent) = parent_id {
                    hierarchy
                        .children_of
                        .entry(parent)
                        .or_default()
                        .push(class_id);
                }

                for member in class.members.iter() {
                    if let ClassMember::Method(method) = member {
                        let method_id = method.name.node;
                        hierarchy
                            .declares_method
                            .insert((class_id, method_id), true);
                        if method.is_final {
                            hierarchy.final_methods.insert((class_id, method_id), true);
                        }
                    }
                }
            }
        }

        hierarchy
    }

    /// Build class hierarchy by scanning all class declarations across multiple modules
    pub fn build_multi_module<'arena>(programs: &[&Program<'arena>]) -> Self {
        let mut hierarchy = ClassHierarchy::default();

        for program in programs {
            for stmt in program.statements.iter() {
                if let Statement::Class(class) = stmt {
                    let class_id = class.name.node;
                    hierarchy.known_classes.insert(class_id, true);
                    hierarchy.is_final.insert(class_id, class.is_final);

                    let parent_id = class.extends.as_ref().and_then(|ext| {
                        if let TypeKind::Reference(type_ref) = &ext.kind {
                            Some(type_ref.name.node)
                        } else {
                            None
                        }
                    });
                    hierarchy.parent_of.insert(class_id, parent_id);

                    if let Some(parent) = parent_id {
                        hierarchy
                            .children_of
                            .entry(parent)
                            .or_default()
                            .push(class_id);
                    }

                    for member in class.members.iter() {
                        if let ClassMember::Method(method) = member {
                            let method_id = method.name.node;
                            hierarchy
                                .declares_method
                                .insert((class_id, method_id), true);
                            if method.is_final {
                                hierarchy.final_methods.insert((class_id, method_id), true);
                            }
                        }
                    }
                }
            }
        }

        // Second pass: collect all instantiations for RTA
        for program in programs {
            hierarchy.collect_instantiations(program);
        }

        // Compute single instantiated subclass for each base class
        hierarchy.compute_single_instantiated_subclasses();

        hierarchy
    }

    /// Collect all `new ClassName()` instantiations from a program
    fn collect_instantiations<'arena>(&mut self, program: &Program<'arena>) {
        for stmt in program.statements.iter() {
            self.collect_instantiations_from_statement(stmt);
        }
    }

    fn collect_instantiations_from_statement<'arena>(&mut self, stmt: &Statement<'arena>) {
        use typedlua_parser::ast::statement::ForStatement;

        match stmt {
            Statement::Function(func) => {
                for s in func.body.statements.iter() {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_instantiations_from_expression(&if_stmt.condition);
                for s in if_stmt.then_block.statements.iter() {
                    self.collect_instantiations_from_statement(s);
                }
                for else_if in if_stmt.else_ifs.iter() {
                    self.collect_instantiations_from_expression(&else_if.condition);
                    for s in else_if.block.statements.iter() {
                        self.collect_instantiations_from_statement(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in else_block.statements.iter() {
                        self.collect_instantiations_from_statement(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.collect_instantiations_from_expression(&while_stmt.condition);
                for s in while_stmt.body.statements.iter() {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::For(for_stmt) => match for_stmt {
                ForStatement::Numeric(for_num) => {
                    self.collect_instantiations_from_expression(&for_num.start);
                    self.collect_instantiations_from_expression(&for_num.end);
                    if let Some(step) = &for_num.step {
                        self.collect_instantiations_from_expression(step);
                    }
                    for s in for_num.body.statements.iter() {
                        self.collect_instantiations_from_statement(s);
                    }
                }
                ForStatement::Generic(for_gen) => {
                    for expr in for_gen.iterators.iter() {
                        self.collect_instantiations_from_expression(expr);
                    }
                    for s in for_gen.body.statements.iter() {
                        self.collect_instantiations_from_statement(s);
                    }
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_instantiations_from_expression(&repeat_stmt.until);
                for s in repeat_stmt.body.statements.iter() {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::Return(ret) => {
                for expr in ret.values.iter() {
                    self.collect_instantiations_from_expression(expr);
                }
            }
            Statement::Variable(var) => {
                self.collect_instantiations_from_expression(&var.initializer);
            }
            Statement::Expression(expr) => {
                self.collect_instantiations_from_expression(expr);
            }
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::Class(class) => {
                for member in class.members.iter() {
                    if let ClassMember::Method(method) = member {
                        if let Some(body) = &method.body {
                            for s in body.statements.iter() {
                                self.collect_instantiations_from_statement(s);
                            }
                        }
                    }
                    if let ClassMember::Constructor(ctor) = member {
                        for s in ctor.body.statements.iter() {
                            self.collect_instantiations_from_statement(s);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_instantiations_from_expression<'arena>(&mut self, expr: &Expression<'arena>) {
        match &expr.kind {
            New(callee, args, _) => {
                self.record_instantiation_from_expression(callee);
                for arg in args.iter() {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            Call(func, args, _) => {
                self.collect_instantiations_from_expression(func);
                for arg in args.iter() {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            MethodCall(obj, _name, args, _) => {
                self.collect_instantiations_from_expression(obj);
                for arg in args.iter() {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            Binary(_op, left, right) => {
                self.collect_instantiations_from_expression(left);
                self.collect_instantiations_from_expression(right);
            }
            Unary(_op, operand) => {
                self.collect_instantiations_from_expression(operand);
            }
            Assignment(left, _op, right) => {
                self.collect_instantiations_from_expression(left);
                self.collect_instantiations_from_expression(right);
            }
            Conditional(cond, then_expr, else_expr) => {
                self.collect_instantiations_from_expression(cond);
                self.collect_instantiations_from_expression(then_expr);
                self.collect_instantiations_from_expression(else_expr);
            }
            Pipe(left, right) => {
                self.collect_instantiations_from_expression(left);
                self.collect_instantiations_from_expression(right);
            }
            Match(match_expr) => {
                self.collect_instantiations_from_expression(match_expr.value);
                for arm in match_expr.arms.iter() {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            self.collect_instantiations_from_expression(e);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            for s in block.statements.iter() {
                                self.collect_instantiations_from_statement(s);
                            }
                        }
                    }
                }
            }
            Arrow(arrow) => {
                for param in arrow.parameters.iter() {
                    if let Some(default) = &param.default {
                        self.collect_instantiations_from_expression(default);
                    }
                }
                match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        self.collect_instantiations_from_expression(e);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        for s in block.statements.iter() {
                            self.collect_instantiations_from_statement(s);
                        }
                    }
                }
            }
            Try(try_expr) => {
                self.collect_instantiations_from_expression(try_expr.expression);
                self.collect_instantiations_from_expression(try_expr.catch_expression);
            }
            ErrorChain(left, right) => {
                self.collect_instantiations_from_expression(left);
                self.collect_instantiations_from_expression(right);
            }
            OptionalMember(obj, _) => {
                self.collect_instantiations_from_expression(obj);
            }
            OptionalIndex(obj, index) => {
                self.collect_instantiations_from_expression(obj);
                self.collect_instantiations_from_expression(index);
            }
            OptionalCall(obj, args, _) => {
                self.collect_instantiations_from_expression(obj);
                for arg in args.iter() {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            OptionalMethodCall(obj, _name, args, _) => {
                self.collect_instantiations_from_expression(obj);
                for arg in args.iter() {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            Member(obj, _) => {
                self.collect_instantiations_from_expression(obj);
            }
            Index(obj, index) => {
                self.collect_instantiations_from_expression(obj);
                self.collect_instantiations_from_expression(index);
            }
            Array(elements) => {
                for elem in elements.iter() {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(e) => {
                            self.collect_instantiations_from_expression(e);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(e) => {
                            self.collect_instantiations_from_expression(e);
                        }
                    }
                }
            }
            Object(props) => {
                for prop in props.iter() {
                    match prop {
                        typedlua_parser::ast::expression::ObjectProperty::Property {
                            value,
                            ..
                        } => {
                            self.collect_instantiations_from_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Computed {
                            key,
                            value,
                            ..
                        } => {
                            self.collect_instantiations_from_expression(key);
                            self.collect_instantiations_from_expression(value);
                        }
                        typedlua_parser::ast::expression::ObjectProperty::Spread {
                            value, ..
                        } => {
                            self.collect_instantiations_from_expression(value);
                        }
                    }
                }
            }
            Parenthesized(inner) => {
                self.collect_instantiations_from_expression(inner);
            }
            _ => {}
        }
    }

    fn record_instantiation_from_expression<'arena>(&mut self, expr: &Expression<'arena>) {
        if let Call(func, _args, _) = &expr.kind {
            self.record_instantiation_from_callee(func);
        }
    }

    fn record_instantiation_from_callee<'arena>(&mut self, expr: &Expression<'arena>) {
        match &expr.kind {
            ExpressionKind::Identifier(id) => {
                let class_id = *id;
                self.classes_with_instantiations.insert(class_id);
                *self.instantiation_counts.entry(class_id).or_insert(0) += 1;
                self.add_instantiation_to_hierarchy(class_id, class_id);
            }
            ExpressionKind::Member(_obj, member) => {
                let class_id = member.node;
                self.classes_with_instantiations.insert(class_id);
                *self.instantiation_counts.entry(class_id).or_insert(0) += 1;
                self.add_instantiation_to_hierarchy(class_id, class_id);
            }
            _ => {}
        }
    }

    fn add_instantiation_to_hierarchy(
        &mut self,
        instantiated_class: StringId,
        original_class: StringId,
    ) {
        let mut current = original_class;
        while let Some(&parent) = self.parent_of.get(&current) {
            if let Some(parent_id) = parent {
                self.instantiated_subclasses
                    .entry(parent_id)
                    .or_default()
                    .insert(instantiated_class);
                current = parent_id;
            } else {
                break;
            }
        }
    }

    fn compute_single_instantiated_subclasses(&mut self) {
        for (base_class, subclasses) in &self.instantiated_subclasses {
            if subclasses.len() == 1 {
                if let Some(&single_subclass) = subclasses.iter().next() {
                    self.single_instantiated_subclass
                        .insert(*base_class, single_subclass);
                    debug!(
                        "RTA: Base class {:?} has single instantiated subclass {:?}",
                        base_class, single_subclass
                    );
                }
            }
        }
    }

    pub fn is_known_class(&self, class: StringId) -> bool {
        self.known_classes.contains_key(&class)
    }

    pub fn can_devirtualize(&self, class: StringId, method: StringId) -> bool {
        if self.is_final.get(&class) == Some(&true) {
            return true;
        }
        if self.final_methods.get(&(class, method)) == Some(&true) {
            return true;
        }
        !self.any_descendant_overrides(class, method)
    }

    fn any_descendant_overrides(&self, class: StringId, method: StringId) -> bool {
        if let Some(children) = self.children_of.get(&class) {
            for &child in children {
                if self.declares_method.get(&(child, method)) == Some(&true) {
                    return true;
                }
                if self.any_descendant_overrides(child, method) {
                    return true;
                }
            }
        }
        false
    }

    pub fn record_instantiation(&mut self, class_name: StringId) {
        self.classes_with_instantiations.insert(class_name);
        *self.instantiation_counts.entry(class_name).or_insert(0) += 1;
    }

    pub fn set_instantiated_subclasses(
        &mut self,
        base_class: StringId,
        subclasses: FxHashSet<StringId>,
    ) {
        self.instantiated_subclasses
            .insert(base_class, subclasses.clone());
        if subclasses.len() == 1 {
            if let Some(&single_subclass) = subclasses.iter().next() {
                self.single_instantiated_subclass
                    .insert(base_class, single_subclass);
            }
        }
    }

    pub fn get_single_instantiated_subclass(&self, class: StringId) -> Option<StringId> {
        self.single_instantiated_subclass.get(&class).copied()
    }

    pub fn has_instantiation_info(&self, class: StringId) -> bool {
        self.instantiated_subclasses.contains_key(&class)
            || self.classes_with_instantiations.contains(&class)
    }

    pub fn can_devirtualize_with_rta(
        &self,
        class: StringId,
        method: StringId,
    ) -> (bool, Option<StringId>) {
        if self.is_final.get(&class) == Some(&true) {
            return (true, None);
        }
        if self.final_methods.get(&(class, method)) == Some(&true) {
            return (true, None);
        }
        if let Some(subclass) = self.single_instantiated_subclass.get(&class) {
            return (true, Some(*subclass));
        }
        let has_overrides = self.any_descendant_overrides(class, method);
        if !has_overrides {
            return (true, None);
        }
        (false, None)
    }
}

// =============================================================================
// DevirtualizationPass — stub during arena migration
// =============================================================================

use crate::config::OptimizationLevel;
use crate::MutableProgram;
use bumpalo::Bump;
use std::sync::Arc;
use typedlua_parser::string_interner::StringInterner;

use super::{AstFeatures, WholeProgramPass};

/// Devirtualization optimization pass (O3).
///
/// Replaces virtual method calls with direct calls when the class hierarchy
/// allows safe devirtualization. Currently a stub during arena migration.
pub struct DevirtualizationPass {
    #[allow(dead_code)]
    interner: Arc<StringInterner>,
    class_hierarchy: Option<ClassHierarchy>,
}

impl DevirtualizationPass {
    pub fn new(interner: Arc<StringInterner>) -> Self {
        Self {
            interner,
            class_hierarchy: None,
        }
    }

    pub fn set_class_hierarchy(&mut self, hierarchy: ClassHierarchy) {
        self.class_hierarchy = Some(hierarchy);
    }
}

impl<'arena> WholeProgramPass<'arena> for DevirtualizationPass {
    fn name(&self) -> &'static str {
        "devirtualization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn required_features(&self) -> AstFeatures {
        AstFeatures::HAS_CLASSES
    }

    fn run(
        &mut self,
        _program: &mut MutableProgram<'arena>,
        _arena: &'arena Bump,
    ) -> Result<bool, String> {
        // TODO: Re-implement devirtualization with arena-allocated AST
        Ok(false)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
