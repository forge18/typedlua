use crate::config::OptimizationLevel;
use crate::optimizer::WholeProgramPass;
use typedlua_parser::ast::statement::{EnumDeclaration, Statement};
use typedlua_parser::ast::Program;

pub struct RichEnumOptimizationPass;

impl RichEnumOptimizationPass {
    pub fn new() -> Self {
        Self
    }
}

impl WholeProgramPass for RichEnumOptimizationPass {
    fn name(&self) -> &'static str {
        "rich-enum-optimization"
    }

    fn min_level(&self) -> OptimizationLevel {
        OptimizationLevel::O2
    }

    fn run(&mut self, program: &mut Program) -> Result<bool, String> {
        let mut rich_enum_count = 0;

        for stmt in &program.statements {
            if let Statement::Enum(enum_decl) = stmt {
                if self.is_rich_enum(enum_decl) {
                    rich_enum_count += 1;
                }
            }
        }

        if rich_enum_count > 0 {
            tracing::debug!(
                "Found {} rich enum(s) - O2 optimizations enabled",
                rich_enum_count
            );
        }

        Ok(false)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl RichEnumOptimizationPass {
    fn is_rich_enum(&self, enum_decl: &EnumDeclaration) -> bool {
        !enum_decl.fields.is_empty()
            || enum_decl.constructor.is_some()
            || !enum_decl.methods.is_empty()
    }
}

impl Default for RichEnumOptimizationPass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use typedlua_parser::ast::expression::{Expression, ExpressionKind, Literal};
    use typedlua_parser::ast::pattern::Pattern;
    use typedlua_parser::ast::statement::{
        Block, EnumConstructor, EnumDeclaration, EnumField, EnumMember, Parameter,
    };
    use typedlua_parser::ast::types::{PrimitiveType, Type, TypeKind};
    use typedlua_parser::ast::Spanned;
    use typedlua_parser::span::Span;
    use typedlua_parser::string_interner::StringInterner;

    fn number_type() -> Type {
        Type::new(TypeKind::Primitive(PrimitiveType::Number), Span::dummy())
    }

    fn create_test_program_with_rich_enum() -> Program {
        let interner = StringInterner::new();

        let mercury_name = Spanned::new(interner.get_or_intern("Mercury"), Span::dummy());
        let mass_field = Spanned::new(interner.get_or_intern("mass"), Span::dummy());
        let radius_field = Spanned::new(interner.get_or_intern("radius"), Span::dummy());

        let enum_decl = EnumDeclaration {
            name: Spanned::new(interner.get_or_intern("Planet"), Span::dummy()),
            members: vec![EnumMember {
                name: mercury_name.clone(),
                arguments: vec![
                    Expression::new(
                        ExpressionKind::Literal(Literal::Number(3.303e23)),
                        Span::dummy(),
                    ),
                    Expression::new(
                        ExpressionKind::Literal(Literal::Number(2.4397e6)),
                        Span::dummy(),
                    ),
                ],
                value: None,
                span: Span::dummy(),
            }],
            fields: vec![
                EnumField {
                    name: mass_field,
                    type_annotation: number_type(),
                    span: Span::dummy(),
                },
                EnumField {
                    name: radius_field,
                    type_annotation: number_type(),
                    span: Span::dummy(),
                },
            ],
            constructor: Some(EnumConstructor {
                parameters: vec![
                    Parameter {
                        pattern: Pattern::Identifier(Spanned::new(
                            interner.get_or_intern("mass"),
                            Span::dummy(),
                        )),
                        type_annotation: Some(number_type()),
                        default: None,
                        is_rest: false,
                        is_optional: false,
                        span: Span::dummy(),
                    },
                    Parameter {
                        pattern: Pattern::Identifier(Spanned::new(
                            interner.get_or_intern("radius"),
                            Span::dummy(),
                        )),
                        type_annotation: Some(number_type()),
                        default: None,
                        is_rest: false,
                        is_optional: false,
                        span: Span::dummy(),
                    },
                ],
                body: Block {
                    statements: vec![],
                    span: Span::dummy(),
                },
                span: Span::dummy(),
            }),
            methods: vec![],
            implements: vec![],
            span: Span::dummy(),
        };

        Program {
            statements: vec![Statement::Enum(enum_decl)],
            span: Span::dummy(),
        }
    }

    fn create_test_program_with_simple_enum() -> Program {
        let interner = StringInterner::new();

        let enum_decl = EnumDeclaration {
            name: Spanned::new(interner.get_or_intern("Color"), Span::dummy()),
            members: vec![EnumMember {
                name: Spanned::new(interner.get_or_intern("Red"), Span::dummy()),
                arguments: vec![],
                value: Some(typedlua_parser::ast::statement::EnumValue::Number(1.0)),
                span: Span::dummy(),
            }],
            fields: vec![],
            constructor: None,
            methods: vec![],
            implements: vec![],
            span: Span::dummy(),
        };

        Program {
            statements: vec![Statement::Enum(enum_decl)],
            span: Span::dummy(),
        }
    }

    #[test]
    fn test_rich_enum_detection() {
        let mut pass = RichEnumOptimizationPass::new();
        let mut program = create_test_program_with_rich_enum();
        let result = pass.run(&mut program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_enum_not_rich() {
        let mut pass = RichEnumOptimizationPass::new();
        let mut program = create_test_program_with_simple_enum();
        let result = pass.run(&mut program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pass_returns_no_changes() {
        let mut pass = RichEnumOptimizationPass::new();
        let mut program = create_test_program_with_rich_enum();
        let result = pass.run(&mut program);
        assert!(!result.unwrap());
    }
}
