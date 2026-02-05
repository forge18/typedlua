//! O3 Devirtualization Pass
//!
//! Converts virtual method calls to direct function calls when the receiver's
//! concrete type is known and it's safe to do so (no polymorphic overrides).
//!
//! This pass performs safety analysis and populates `receiver_class` on method
//! calls that can be safely devirtualized. The actual transformation is handled
//! by the O2 `MethodToFunctionConversionPass`.

use crate::config::OptimizationLevel;

use crate::optimizer::WholeProgramPass;
use rustc_hash::FxHashMap;
use std::rc::Rc;
use typedlua_parser::ast::expression::{Expression, ExpressionKind, ReceiverClassInfo};
use typedlua_parser::ast::statement::{ClassMember, Statement};
use typedlua_parser::ast::types::TypeKind;
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::{StringId, StringInterner};

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
}

/// O3 Devirtualization pass
///
/// Analyzes method calls and marks safe ones for devirtualization by setting
/// the `receiver_class` field. The actual transformation is performed by
/// the O2 `MethodToFunctionConversionPass`.
pub struct DevirtualizationPass;

impl DevirtualizationPass {
    pub fn new(_interner: Rc<StringInterner>) -> Self {
        Self
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

                                // Check if devirtualization is safe
                                if hierarchy.can_devirtualize(class_id, method_id) {
                                    expr.receiver_class = Some(ReceiverClassInfo {
                                        class_name: class_id,
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
        // Build class hierarchy for safety analysis
        let hierarchy = ClassHierarchy::build(program);

        // Process all statements
        let mut changed = false;
        for stmt in &mut program.statements {
            changed |= self.process_statement(stmt, &hierarchy);
        }

        Ok(changed)
    }
}

impl Default for DevirtualizationPass {
    fn default() -> Self {
        Self
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
}
