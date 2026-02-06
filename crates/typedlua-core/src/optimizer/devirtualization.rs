//! O3 Devirtualization Pass
//!
//! Converts virtual method calls to direct function calls when the receiver's
//! concrete type is known and it's safe to do so (no polymorphic overrides).
//!
//! This pass performs safety analysis and populates `receiver_class` on method
//! calls that can be safely devirtualized. The actual transformation is handled
//! by the O2 `MethodToFunctionConversionPass`.
//!
//! RTA (Rapid Type Analysis) Devirtualization:
//! When only one subclass of a class is ever instantiated, we can safely
//! devirtualize calls on variables of the base class type.

use crate::config::OptimizationLevel;

use crate::optimizer::WholeProgramPass;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use tracing::debug;
use typedlua_parser::ast::expression::{Expression, ExpressionKind, ReceiverClassInfo};
use typedlua_parser::ast::statement::{ClassMember, Statement};
use typedlua_parser::ast::types::TypeKind;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::{StringId, StringInterner};

use ExpressionKind::*;

/// Class hierarchy information for devirtualization safety analysis
#[derive(Debug, Default)]
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
    /// For RTA devirtualization: if only one subclass is instantiated,
    /// we can devirtualize calls on variables of the base class type
    instantiated_subclasses: FxHashMap<StringId, FxHashSet<StringId>>,
    /// RTA: For each class, the single instantiated subclass if there's exactly one
    /// This allows fast lookup for devirtualization
    single_instantiated_subclass: FxHashMap<StringId, StringId>,
    /// RTA: Total count of instantiations per class (sum of all subclass instantiations)
    instantiation_counts: FxHashMap<StringId, usize>,
    /// RTA: Set of all classes that have any instantiations
    classes_with_instantiations: FxHashSet<StringId>,
}

impl ClassHierarchy {
    /// Build class hierarchy by scanning all class declarations in the program
    pub fn build(program: &Program) -> Self {
        let mut hierarchy = ClassHierarchy::default();

        // First pass: collect all classes and their parent relationships
        for stmt in &program.statements {
            if let Statement::Class(class) = stmt {
                let class_id = class.name.node;

                // Mark as known class
                hierarchy.known_classes.insert(class_id, true);

                // Record finality
                hierarchy.is_final.insert(class_id, class.is_final);

                // Record parent relationship
                let parent_id = class.extends.as_ref().and_then(|ext| {
                    if let TypeKind::Reference(type_ref) = &ext.kind {
                        Some(type_ref.name.node)
                    } else {
                        None
                    }
                });
                hierarchy.parent_of.insert(class_id, parent_id);

                // Add to parent's children list
                if let Some(parent) = parent_id {
                    hierarchy
                        .children_of
                        .entry(parent)
                        .or_default()
                        .push(class_id);
                }

                // Record methods declared in this class
                for member in &class.members {
                    if let ClassMember::Method(method) = member {
                        let method_id = method.name.node;
                        hierarchy
                            .declares_method
                            .insert((class_id, method_id), true);

                        // Record method finality
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
    ///
    /// This enables whole-program analysis for cross-module devirtualization.
    /// Each program's AST is scanned to build a complete class hierarchy that
    /// spans module boundaries.
    pub fn build_multi_module(programs: &[&Program]) -> Self {
        let mut hierarchy = ClassHierarchy::default();

        // Scan all modules to build complete hierarchy
        for program in programs {
            for stmt in &program.statements {
                if let Statement::Class(class) = stmt {
                    let class_id = class.name.node;

                    // Mark as known class
                    hierarchy.known_classes.insert(class_id, true);

                    // Record finality
                    hierarchy.is_final.insert(class_id, class.is_final);

                    // Record parent relationship
                    let parent_id = class.extends.as_ref().and_then(|ext| {
                        if let TypeKind::Reference(type_ref) = &ext.kind {
                            Some(type_ref.name.node)
                        } else {
                            None
                        }
                    });
                    hierarchy.parent_of.insert(class_id, parent_id);

                    // Add to parent's children list
                    if let Some(parent) = parent_id {
                        hierarchy
                            .children_of
                            .entry(parent)
                            .or_default()
                            .push(class_id);
                    }

                    // Record methods declared in this class
                    for member in &class.members {
                        if let ClassMember::Method(method) = member {
                            let method_id = method.name.node;
                            hierarchy
                                .declares_method
                                .insert((class_id, method_id), true);

                            // Record method finality
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
    fn collect_instantiations(&mut self, program: &Program) {
        for stmt in &program.statements {
            self.collect_instantiations_from_statement(stmt);
        }
    }

    fn collect_instantiations_from_statement(&mut self, stmt: &Statement) {
        use typedlua_parser::ast::statement::ForStatement;

        match stmt {
            Statement::Function(func) => {
                for s in &func.body.statements {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_instantiations_from_expression(&if_stmt.condition);
                for s in &if_stmt.then_block.statements {
                    self.collect_instantiations_from_statement(s);
                }
                for else_if in &if_stmt.else_ifs {
                    self.collect_instantiations_from_expression(&else_if.condition);
                    for s in &else_if.block.statements {
                        self.collect_instantiations_from_statement(s);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for s in &else_block.statements {
                        self.collect_instantiations_from_statement(s);
                    }
                }
            }
            Statement::While(while_stmt) => {
                self.collect_instantiations_from_expression(&while_stmt.condition);
                for s in &while_stmt.body.statements {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::For(for_stmt) => match for_stmt.as_ref() {
                ForStatement::Numeric(for_num) => {
                    self.collect_instantiations_from_expression(&for_num.start);
                    self.collect_instantiations_from_expression(&for_num.end);
                    if let Some(step) = &for_num.step {
                        self.collect_instantiations_from_expression(step);
                    }
                    for s in &for_num.body.statements {
                        self.collect_instantiations_from_statement(s);
                    }
                }
                ForStatement::Generic(for_gen) => {
                    for expr in &for_gen.iterators {
                        self.collect_instantiations_from_expression(expr);
                    }
                    for s in &for_gen.body.statements {
                        self.collect_instantiations_from_statement(s);
                    }
                }
            },
            Statement::Repeat(repeat_stmt) => {
                self.collect_instantiations_from_expression(&repeat_stmt.until);
                for s in &repeat_stmt.body.statements {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::Return(ret) => {
                for expr in &ret.values {
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
                for s in &block.statements {
                    self.collect_instantiations_from_statement(s);
                }
            }
            Statement::Class(class) => {
                for member in &class.members {
                    if let ClassMember::Method(method) = member {
                        if let Some(body) = &method.body {
                            for s in &body.statements {
                                self.collect_instantiations_from_statement(s);
                            }
                        }
                    }
                    if let ClassMember::Constructor(ctor) = member {
                        for s in &ctor.body.statements {
                            self.collect_instantiations_from_statement(s);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_instantiations_from_expression(&mut self, expr: &Expression) {
        use ExpressionKind::*;

        match &expr.kind {
            New(callee, args, _) => {
                // Record this instantiation
                self.record_instantiation_from_expression(callee);
                for arg in args {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            Call(func, args, _) => {
                self.collect_instantiations_from_expression(func);
                for arg in args {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            MethodCall(obj, _name, args, _) => {
                self.collect_instantiations_from_expression(obj);
                for arg in args {
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
                self.collect_instantiations_from_expression(&match_expr.value);
                for arm in &match_expr.arms {
                    match &arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            self.collect_instantiations_from_expression(e);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            for s in &block.statements {
                                self.collect_instantiations_from_statement(s);
                            }
                        }
                    }
                }
            }
            Arrow(arrow) => {
                for param in &arrow.parameters {
                    if let Some(default) = &param.default {
                        self.collect_instantiations_from_expression(default);
                    }
                }
                match &arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        self.collect_instantiations_from_expression(e);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        for s in &block.statements {
                            self.collect_instantiations_from_statement(s);
                        }
                    }
                }
            }
            Try(try_expr) => {
                self.collect_instantiations_from_expression(&try_expr.expression);
                self.collect_instantiations_from_expression(&try_expr.catch_expression);
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
                for arg in args {
                    self.collect_instantiations_from_expression(&arg.value);
                }
            }
            OptionalMethodCall(obj, _name, args, _) => {
                self.collect_instantiations_from_expression(obj);
                for arg in args {
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
                for elem in elements {
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
                for prop in props {
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

    fn record_instantiation_from_expression(&mut self, expr: &Expression) {
        // The callee of a `new` expression is typically a Call expression like `Foo(args)`
        // We need to extract the class name from it
        if let Call(func, _args, _) = &expr.kind {
            self.record_instantiation_from_callee(func);
        }
    }

    fn record_instantiation_from_callee(&mut self, expr: &Expression) {
        match &expr.kind {
            ExpressionKind::Identifier(id) => {
                let class_id = *id;
                // Mark class as having instantiations
                self.classes_with_instantiations.insert(class_id);
                *self.instantiation_counts.entry(class_id).or_insert(0) += 1;

                // Also record this for all parent classes (RTA needs to know instantiations at each level)
                self.add_instantiation_to_hierarchy(class_id, class_id);
            }
            ExpressionKind::Member(_obj, member) => {
                // For now, just record the simple name
                let class_id = member.node;
                self.classes_with_instantiations.insert(class_id);
                *self.instantiation_counts.entry(class_id).or_insert(0) += 1;
                self.add_instantiation_to_hierarchy(class_id, class_id);
            }
            _ => {}
        }
    }

    /// Add an instantiation to all ancestors in the hierarchy
    fn add_instantiation_to_hierarchy(
        &mut self,
        instantiated_class: StringId,
        original_class: StringId,
    ) {
        // Walk up the parent chain and record that original_class has instantiations of instantiated_class
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

    /// Compute single_instantiated_subclass for all base classes
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

    /// Check if a class is known (vs an interface)
    pub fn is_known_class(&self, class: StringId) -> bool {
        self.known_classes.contains_key(&class)
    }

    /// Check if a method call can be safely devirtualized
    ///
    /// Safe if:
    /// 1. Class is final, OR
    /// 2. Method is final, OR
    /// 3. No descendant class overrides this method
    pub fn can_devirtualize(&self, class: StringId, method: StringId) -> bool {
        // If class is final, always safe
        if self.is_final.get(&class) == Some(&true) {
            return true;
        }

        // If method is final, always safe
        if self.final_methods.get(&(class, method)) == Some(&true) {
            return true;
        }

        // Check if any descendant overrides this method
        !self.any_descendant_overrides(class, method)
    }

    /// Recursively check if any descendant class declares (overrides) the method
    fn any_descendant_overrides(&self, class: StringId, method: StringId) -> bool {
        if let Some(children) = self.children_of.get(&class) {
            for &child in children {
                // Check if child declares this method (override)
                if self.declares_method.get(&(child, method)) == Some(&true) {
                    return true;
                }
                // Recursively check grandchildren
                if self.any_descendant_overrides(child, method) {
                    return true;
                }
            }
        }
        false
    }

    /// RTA: Record that a specific class was instantiated
    pub fn record_instantiation(&mut self, class_name: StringId) {
        self.classes_with_instantiations.insert(class_name);
        *self.instantiation_counts.entry(class_name).or_insert(0) += 1;
    }

    /// RTA: Set the instantiated subclasses for a base class
    /// Called after multi-module analysis to set the complete instantiation map
    pub fn set_instantiated_subclasses(
        &mut self,
        base_class: StringId,
        subclasses: FxHashSet<StringId>,
    ) {
        self.instantiated_subclasses
            .insert(base_class, subclasses.clone());

        // If exactly one subclass is instantiated, cache it for fast lookup
        if subclasses.len() == 1 {
            if let Some(&single_subclass) = subclasses.iter().next() {
                self.single_instantiated_subclass
                    .insert(base_class, single_subclass);
            }
        }
    }

    /// RTA: Check if this class has exactly one instantiated subclass
    /// If so, return that subclass - enabling devirtualization
    pub fn get_single_instantiated_subclass(&self, class: StringId) -> Option<StringId> {
        self.single_instantiated_subclass.get(&class).copied()
    }

    /// RTA: Check if a class or any of its ancestors have instantiation info
    pub fn has_instantiation_info(&self, class: StringId) -> bool {
        self.instantiated_subclasses.contains_key(&class)
            || self.classes_with_instantiations.contains(&class)
    }

    /// Check if a method call can be safely devirtualized using RTA
    ///
    /// Safe if:
    /// 1. Class is final, OR
    /// 2. Method is final, OR
    /// 3. No descendant class overrides this method, OR
    /// 4. (RTA) Only one subclass is ever instantiated -> devirtualize to that subclass
    pub fn can_devirtualize_with_rta(
        &self,
        class: StringId,
        method: StringId,
    ) -> (bool, Option<StringId>) {
        // If class is final, always safe
        if self.is_final.get(&class) == Some(&true) {
            debug!("  RTA: Class {:?} is final, can devirtualize", class);
            return (true, None);
        }

        // If method is final, always safe
        if self.final_methods.get(&(class, method)) == Some(&true) {
            debug!(
                "  RTA: Method {:?} on class {:?} is final, can devirtualize",
                method, class
            );
            return (true, None);
        }

        // RTA check: If exactly one subclass is instantiated, we can devirtualize
        if let Some(subclass) = self.single_instantiated_subclass.get(&class) {
            debug!(
                "  RTA: Class {:?} has only one instantiated subclass {:?}, can devirtualize",
                class, subclass
            );
            return (true, Some(*subclass));
        }

        // Check if any descendant overrides this method
        let has_overrides = self.any_descendant_overrides(class, method);
        if !has_overrides {
            debug!(
                "  RTA: No descendants of {:?} override method {:?}, can devirtualize",
                class, method
            );
            return (true, None);
        }

        debug!(
            "  RTA: Cannot devirtualize {:?}.{:?} - has overriding descendants",
            class, method
        );
        (false, None)
    }
}

/// O3 Devirtualization pass
///
/// Analyzes method calls and marks safe ones for devirtualization by setting
/// the `receiver_class` field. The actual transformation is performed by
/// the O2 `MethodToFunctionConversionPass`.
pub struct DevirtualizationPass {
    /// Pre-built class hierarchy from whole-program analysis (optional)
    class_hierarchy: Option<Arc<ClassHierarchy>>,
}

impl DevirtualizationPass {
    pub fn new(_interner: Arc<StringInterner>) -> Self {
        Self {
            class_hierarchy: None,
        }
    }

    /// Set the pre-built class hierarchy from whole-program analysis
    ///
    /// When set, this hierarchy will be used instead of building one from the
    /// single module being optimized, enabling cross-module devirtualization.
    pub fn set_class_hierarchy(&mut self, hierarchy: Arc<ClassHierarchy>) {
        self.class_hierarchy = Some(hierarchy);
    }

    /// Process a statement, looking for method calls to devirtualize
    fn process_statement(&self, stmt: &mut Statement, hierarchy: &ClassHierarchy) -> bool {
        match stmt {
            Statement::Function(func) => {
                let mut changed = false;
                for s in &mut func.body.statements {
                    changed |= self.process_statement(s, hierarchy);
                }
                changed
            }
            Statement::If(if_stmt) => {
                let mut changed = self.process_expression(&mut if_stmt.condition, hierarchy);
                changed |= self.process_block(&mut if_stmt.then_block, hierarchy);
                for else_if in &mut if_stmt.else_ifs {
                    changed |= self.process_expression(&mut else_if.condition, hierarchy);
                    changed |= self.process_block(&mut else_if.block, hierarchy);
                }
                if let Some(else_block) = &mut if_stmt.else_block {
                    changed |= self.process_block(else_block, hierarchy);
                }
                changed
            }
            Statement::While(while_stmt) => {
                let mut changed = self.process_expression(&mut while_stmt.condition, hierarchy);
                changed |= self.process_block(&mut while_stmt.body, hierarchy);
                changed
            }
            Statement::For(for_stmt) => {
                use typedlua_parser::ast::statement::ForStatement;
                let body = match &mut **for_stmt {
                    ForStatement::Numeric(for_num) => &mut for_num.body,
                    ForStatement::Generic(for_gen) => &mut for_gen.body,
                };
                self.process_block(body, hierarchy)
            }
            Statement::Repeat(repeat_stmt) => {
                let mut changed = self.process_expression(&mut repeat_stmt.until, hierarchy);
                changed |= self.process_block(&mut repeat_stmt.body, hierarchy);
                changed
            }
            Statement::Return(return_stmt) => {
                let mut changed = false;
                for value in &mut return_stmt.values {
                    changed |= self.process_expression(value, hierarchy);
                }
                changed
            }
            Statement::Expression(expr) => self.process_expression(expr, hierarchy),
            Statement::Block(block) => self.process_block(block, hierarchy),
            Statement::Try(try_stmt) => {
                let mut changed = self.process_block(&mut try_stmt.try_block, hierarchy);
                for clause in &mut try_stmt.catch_clauses {
                    changed |= self.process_block(&mut clause.body, hierarchy);
                }
                if let Some(finally) = &mut try_stmt.finally_block {
                    changed |= self.process_block(finally, hierarchy);
                }
                changed
            }
            Statement::Variable(decl) => self.process_expression(&mut decl.initializer, hierarchy),
            Statement::Class(class) => {
                let mut changed = false;
                for member in &mut class.members {
                    match member {
                        ClassMember::Method(method) => {
                            if let Some(body) = &mut method.body {
                                changed |= self.process_block(body, hierarchy);
                            }
                        }
                        ClassMember::Constructor(ctor) => {
                            changed |= self.process_block(&mut ctor.body, hierarchy);
                        }
                        ClassMember::Getter(getter) => {
                            changed |= self.process_block(&mut getter.body, hierarchy);
                        }
                        ClassMember::Setter(setter) => {
                            changed |= self.process_block(&mut setter.body, hierarchy);
                        }
                        ClassMember::Operator(op) => {
                            changed |= self.process_block(&mut op.body, hierarchy);
                        }
                        ClassMember::Property(_) => {}
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn process_block(
        &self,
        block: &mut typedlua_parser::ast::statement::Block,
        hierarchy: &ClassHierarchy,
    ) -> bool {
        let mut changed = false;
        for stmt in &mut block.statements {
            changed |= self.process_statement(stmt, hierarchy);
        }
        changed
    }

    fn process_expression(&self, expr: &mut Expression, hierarchy: &ClassHierarchy) -> bool {
        match &mut expr.kind {
            ExpressionKind::MethodCall(obj, method_name, args, _) => {
                let mut changed = self.process_expression(obj, hierarchy);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value, hierarchy);
                }

                // Only process if receiver_class is not already set
                if expr.receiver_class.is_none() {
                    // Check if the receiver has an annotated type that's a class reference
                    if let Some(ref obj_type) = obj.annotated_type {
                        if let TypeKind::Reference(type_ref) = &obj_type.kind {
                            let class_id = type_ref.name.node;

                            // Verify it's a class (not interface)
                            if hierarchy.is_known_class(class_id) {
                                let method_id = method_name.node;

                                // Check if devirtualization is safe using RTA
                                let (can_devirt, rta_subclass) =
                                    hierarchy.can_devirtualize_with_rta(class_id, method_id);

                                if can_devirt {
                                    // Use RTA subclass if available, otherwise use the annotated type
                                    let target_class = rta_subclass.unwrap_or(class_id);

                                    debug!(
                                        "Devirtualizing {:?}.{:?} -> {:?} (RTA: {})",
                                        class_id,
                                        method_id,
                                        target_class,
                                        rta_subclass.is_some()
                                    );

                                    expr.receiver_class = Some(ReceiverClassInfo {
                                        class_name: target_class,
                                        is_static: false,
                                    });
                                    changed = true;
                                }
                            }
                        }
                    }
                }

                changed
            }
            ExpressionKind::Call(func, args, _) => {
                let mut changed = self.process_expression(func, hierarchy);
                for arg in args.iter_mut() {
                    changed |= self.process_expression(&mut arg.value, hierarchy);
                }
                changed
            }
            ExpressionKind::Binary(_op, left, right) => {
                let mut changed = self.process_expression(left, hierarchy);
                changed |= self.process_expression(right, hierarchy);
                changed
            }
            ExpressionKind::Unary(_op, operand) => self.process_expression(operand, hierarchy),
            ExpressionKind::Assignment(left, _op, right) => {
                let mut changed = self.process_expression(left, hierarchy);
                changed |= self.process_expression(right, hierarchy);
                changed
            }
            ExpressionKind::Conditional(cond, then_expr, else_expr) => {
                let mut changed = self.process_expression(cond, hierarchy);
                changed |= self.process_expression(then_expr, hierarchy);
                changed |= self.process_expression(else_expr, hierarchy);
                changed
            }
            ExpressionKind::Pipe(left, right) => {
                let mut changed = self.process_expression(left, hierarchy);
                changed |= self.process_expression(right, hierarchy);
                changed
            }
            ExpressionKind::Match(match_expr) => {
                let mut changed = self.process_expression(&mut match_expr.value, hierarchy);
                for arm in &mut match_expr.arms {
                    match &mut arm.body {
                        typedlua_parser::ast::expression::MatchArmBody::Expression(e) => {
                            changed |= self.process_expression(e, hierarchy);
                        }
                        typedlua_parser::ast::expression::MatchArmBody::Block(block) => {
                            changed |= self.process_block(block, hierarchy);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Arrow(arrow) => {
                let mut changed = false;
                for param in &mut arrow.parameters {
                    if let Some(default) = &mut param.default {
                        changed |= self.process_expression(default, hierarchy);
                    }
                }
                match &mut arrow.body {
                    typedlua_parser::ast::expression::ArrowBody::Expression(e) => {
                        changed |= self.process_expression(e, hierarchy);
                    }
                    typedlua_parser::ast::expression::ArrowBody::Block(block) => {
                        changed |= self.process_block(block, hierarchy);
                    }
                }
                changed
            }
            ExpressionKind::New(callee, args, _) => {
                let mut changed = self.process_expression(callee, hierarchy);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, hierarchy);
                }
                changed
            }
            ExpressionKind::Try(try_expr) => {
                let mut changed = self.process_expression(&mut try_expr.expression, hierarchy);
                changed |= self.process_expression(&mut try_expr.catch_expression, hierarchy);
                changed
            }
            ExpressionKind::ErrorChain(left, right) => {
                let mut changed = self.process_expression(left, hierarchy);
                changed |= self.process_expression(right, hierarchy);
                changed
            }
            ExpressionKind::OptionalMember(obj, _) => self.process_expression(obj, hierarchy),
            ExpressionKind::OptionalIndex(obj, index) => {
                let mut changed = self.process_expression(obj, hierarchy);
                changed |= self.process_expression(index, hierarchy);
                changed
            }
            ExpressionKind::OptionalCall(obj, args, _) => {
                let mut changed = self.process_expression(obj, hierarchy);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, hierarchy);
                }
                changed
            }
            ExpressionKind::OptionalMethodCall(obj, _method_name, args, _) => {
                let mut changed = self.process_expression(obj, hierarchy);
                for arg in args {
                    changed |= self.process_expression(&mut arg.value, hierarchy);
                }
                // Don't devirtualize optional method calls - they have different semantics
                changed
            }
            ExpressionKind::Member(obj, _) => self.process_expression(obj, hierarchy),
            ExpressionKind::Index(obj, index) => {
                let mut changed = self.process_expression(obj, hierarchy);
                changed |= self.process_expression(index, hierarchy);
                changed
            }
            ExpressionKind::Array(elements) => {
                let mut changed = false;
                for elem in elements {
                    match elem {
                        typedlua_parser::ast::expression::ArrayElement::Expression(expr) => {
                            changed |= self.process_expression(expr, hierarchy);
                        }
                        typedlua_parser::ast::expression::ArrayElement::Spread(expr) => {
                            changed |= self.process_expression(expr, hierarchy);
                        }
                    }
                }
                changed
            }
            ExpressionKind::Object(props) => {
                let mut changed = false;
                for prop in props {
                    if let typedlua_parser::ast::expression::ObjectProperty::Property {
                        value,
                        ..
                    } = prop
                    {
                        changed |= self.process_expression(value, hierarchy);
                    }
                }
                changed
            }
            ExpressionKind::Parenthesized(inner) => self.process_expression(inner, hierarchy),
            // Terminal expressions - no recursion needed
            ExpressionKind::Identifier(_)
            | ExpressionKind::Literal(_)
            | ExpressionKind::SelfKeyword
            | ExpressionKind::SuperKeyword
            | ExpressionKind::Template(_)
            | ExpressionKind::TypeAssertion(_, _)
            | ExpressionKind::Function(_) => false,
        }
    }
}

impl WholeProgramPass for DevirtualizationPass {
    fn name(&self) -> &'static str {
        "devirtualization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O3
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        // Use pre-built hierarchy if available (cross-module analysis),
        // otherwise build from this module only (single-module analysis)
        let hierarchy = if let Some(ref hierarchy) = self.class_hierarchy {
            // Use the pre-built cross-module hierarchy
            hierarchy.clone()
        } else {
            // Build hierarchy from this module only
            Arc::new(ClassHierarchy::build(program))
        };

        // Process all statements
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt, &hierarchy);
        }

        Ok(changed)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for DevirtualizationPass {
    fn default() -> Self {
        Self {
            class_hierarchy: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_hierarchy_build() {
        // Simple test that hierarchy builds without crashing
        let program = Program::new(vec![], typedlua_parser::span::Span::dummy());
        let hierarchy = ClassHierarchy::build(&program);
        assert!(hierarchy.known_classes.is_empty());
    }

    #[test]
    fn test_can_devirtualize_final_class() {
        let mut hierarchy = ClassHierarchy::default();
        let class_id = StringId::from_u32(1);
        let method_id = StringId::from_u32(2);

        hierarchy.known_classes.insert(class_id, true);
        hierarchy.is_final.insert(class_id, true);

        assert!(hierarchy.can_devirtualize(class_id, method_id));
    }

    #[test]
    fn test_can_devirtualize_final_method() {
        let mut hierarchy = ClassHierarchy::default();
        let class_id = StringId::from_u32(1);
        let method_id = StringId::from_u32(2);

        hierarchy.known_classes.insert(class_id, true);
        hierarchy.is_final.insert(class_id, false);
        hierarchy.final_methods.insert((class_id, method_id), true);

        assert!(hierarchy.can_devirtualize(class_id, method_id));
    }

    #[test]
    fn test_cannot_devirtualize_overridden_method() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child_id = StringId::from_u32(2);
        let method_id = StringId::from_u32(3);

        hierarchy.known_classes.insert(parent_id, true);
        hierarchy.known_classes.insert(child_id, true);
        hierarchy.is_final.insert(parent_id, false);
        hierarchy.is_final.insert(child_id, false);
        hierarchy.parent_of.insert(child_id, Some(parent_id));
        hierarchy.children_of.insert(parent_id, vec![child_id]);
        // Child overrides the method
        hierarchy
            .declares_method
            .insert((child_id, method_id), true);

        // Should NOT be able to devirtualize parent's method call
        assert!(!hierarchy.can_devirtualize(parent_id, method_id));
    }

    #[test]
    fn test_can_devirtualize_non_overridden_method() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child_id = StringId::from_u32(2);
        let method_id = StringId::from_u32(3);

        hierarchy.known_classes.insert(parent_id, true);
        hierarchy.known_classes.insert(child_id, true);
        hierarchy.is_final.insert(parent_id, false);
        hierarchy.is_final.insert(child_id, false);
        hierarchy.parent_of.insert(child_id, Some(parent_id));
        hierarchy.children_of.insert(parent_id, vec![child_id]);
        // Child does NOT override the method - no declares_method entry

        // Should be able to devirtualize
        assert!(hierarchy.can_devirtualize(parent_id, method_id));
    }

    #[test]
    fn test_rta_single_instantiation_enables_devirtualization() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child_id = StringId::from_u32(2);
        let method_id = StringId::from_u32(3);

        hierarchy.known_classes.insert(parent_id, true);
        hierarchy.known_classes.insert(child_id, true);
        hierarchy.is_final.insert(parent_id, false);
        hierarchy.is_final.insert(child_id, false);
        hierarchy.parent_of.insert(child_id, Some(parent_id));
        hierarchy.children_of.insert(parent_id, vec![child_id]);
        hierarchy
            .declares_method
            .insert((child_id, method_id), true);

        // RTA: Only one subclass is instantiated (Child)
        let mut subclasses = FxHashSet::default();
        subclasses.insert(child_id);
        hierarchy.set_instantiated_subclasses(parent_id, subclasses);

        // Even though child overrides the method, RTA says only Child is instantiated
        // So we should be able to devirtualize to Child
        let (can_devirt, subclass) = hierarchy.can_devirtualize_with_rta(parent_id, method_id);
        assert!(can_devirt);
        assert_eq!(subclass, Some(child_id));
    }

    #[test]
    fn test_rta_multi_instantiation_prevents_devirtualization() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child1_id = StringId::from_u32(2);
        let child2_id = StringId::from_u32(3);
        let method_id = StringId::from_u32(4);

        hierarchy.known_classes.insert(parent_id, true);
        hierarchy.known_classes.insert(child1_id, true);
        hierarchy.known_classes.insert(child2_id, true);
        hierarchy.is_final.insert(parent_id, false);
        hierarchy.is_final.insert(child1_id, false);
        hierarchy.is_final.insert(child2_id, false);
        hierarchy.parent_of.insert(child1_id, Some(parent_id));
        hierarchy.parent_of.insert(child2_id, Some(parent_id));
        hierarchy
            .children_of
            .insert(parent_id, vec![child1_id, child2_id]);
        hierarchy
            .declares_method
            .insert((child1_id, method_id), true);
        hierarchy
            .declares_method
            .insert((child2_id, method_id), true);

        // RTA: Multiple subclasses are instantiated
        let mut subclasses = FxHashSet::default();
        subclasses.insert(child1_id);
        subclasses.insert(child2_id);
        hierarchy.set_instantiated_subclasses(parent_id, subclasses);

        // Should NOT be able to devirtualize because both children are instantiated
        let (can_devirt, subclass) = hierarchy.can_devirtualize_with_rta(parent_id, method_id);
        assert!(!can_devirt);
        assert_eq!(subclass, None);
    }

    #[test]
    fn test_rta_no_instantiations_falls_back_to_cha() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child_id = StringId::from_u32(2);
        let method_id = StringId::from_u32(3);

        hierarchy.known_classes.insert(parent_id, true);
        hierarchy.known_classes.insert(child_id, true);
        hierarchy.is_final.insert(parent_id, false);
        hierarchy.is_final.insert(child_id, false);
        hierarchy.parent_of.insert(child_id, Some(parent_id));
        hierarchy.children_of.insert(parent_id, vec![child_id]);
        // Child does NOT override the method
        hierarchy
            .declares_method
            .insert((child_id, method_id), false);

        // No instantiations recorded - should fall back to CHA
        let (can_devirt, subclass) = hierarchy.can_devirtualize_with_rta(parent_id, method_id);
        assert!(can_devirt);
        assert_eq!(subclass, None);
    }

    #[test]
    fn test_rta_instantiation_counts() {
        let mut hierarchy = ClassHierarchy::default();
        let class_id = StringId::from_u32(1);

        hierarchy.record_instantiation(class_id);
        hierarchy.record_instantiation(class_id);
        hierarchy.record_instantiation(class_id);

        assert_eq!(hierarchy.instantiation_counts.get(&class_id), Some(&3));
        assert!(hierarchy.classes_with_instantiations.contains(&class_id));
    }

    #[test]
    fn test_rta_compute_single_instantiated_subclass() {
        let mut hierarchy = ClassHierarchy::default();
        let parent_id = StringId::from_u32(1);
        let child_id = StringId::from_u32(2);

        let mut subclasses = FxHashSet::default();
        subclasses.insert(child_id);
        hierarchy.set_instantiated_subclasses(parent_id, subclasses);

        hierarchy.compute_single_instantiated_subclasses();

        assert_eq!(
            hierarchy.single_instantiated_subclass.get(&parent_id),
            Some(&child_id)
        );
    }
}
