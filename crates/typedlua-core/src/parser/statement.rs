use super::{ExpressionParser, Parser, ParserError, PatternParser, TypeParser};
use crate::ast::statement::*;
use crate::ast::types::{PrimitiveType, Type, TypeKind};
use crate::ast::Ident;
use crate::ast::Spanned;
use crate::lexer::TokenKind;
use crate::span::Span;

pub trait StatementParser {
    fn parse_statement(&mut self) -> Result<Statement, ParserError>;
    fn parse_block(&mut self) -> Result<Block, ParserError>;
}

impl StatementParser for Parser<'_> {
    fn parse_statement(&mut self) -> Result<Statement, ParserError> {
        // Check for decorators first
        if self.check(&TokenKind::At) {
            return self.parse_class_declaration();
        }

        match &self.current().kind {
            TokenKind::Const | TokenKind::Local => self.parse_variable_declaration(),
            TokenKind::Function => self.parse_function_declaration(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Repeat => self.parse_repeat_statement(),
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::Break => {
                let span = self.current_span();
                self.advance();
                Ok(Statement::Break(span))
            }
            TokenKind::Continue => {
                let span = self.current_span();
                self.advance();
                Ok(Statement::Continue(span))
            }
            TokenKind::Interface => self.parse_interface_declaration(),
            TokenKind::Type => self.parse_type_alias_declaration(),
            TokenKind::Enum => self.parse_enum_declaration(),
            TokenKind::Import => self.parse_import_declaration(),
            TokenKind::Export => self.parse_export_declaration(),
            TokenKind::Abstract | TokenKind::Final | TokenKind::Class => {
                self.parse_class_declaration()
            }
            TokenKind::Declare => self.parse_declare_statement(),
            TokenKind::Throw => self.parse_throw_statement(),
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Rethrow => self.parse_rethrow_statement(),
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParserError> {
        let start_span = self.current_span();
        let mut statements = Vec::new();

        // Blocks in Lua don't require braces in many contexts
        // We'll parse until we hit an end marker
        while !self.is_at_end()
            && !matches!(
                &self.current().kind,
                TokenKind::End
                    | TokenKind::Else
                    | TokenKind::Elseif
                    | TokenKind::Until
                    | TokenKind::RightBrace
            )
        {
            statements.push(self.parse_statement()?);
        }

        let end_span = if !statements.is_empty() {
            statements.last().unwrap().span()
        } else {
            start_span
        };

        Ok(Block {
            statements,
            span: start_span.combine(&end_span),
        })
    }
}

// Statement implementations
impl Parser<'_> {
    fn parse_variable_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        let kind = if matches!(self.current().kind, TokenKind::Const) {
            VariableKind::Const
        } else {
            VariableKind::Local
        };
        self.advance();

        let pattern = self.parse_pattern()?;

        let type_annotation = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(TokenKind::Equal, "Expected '=' in variable declaration")?;

        let initializer = self.parse_expression()?;
        let end_span = initializer.span;

        Ok(Statement::Variable(VariableDeclaration {
            kind,
            pattern,
            type_annotation,
            initializer,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_function_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Function, "Expected 'function'")?;

        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        self.consume(TokenKind::LeftParen, "Expected '(' after function name")?;
        let parameters = self.parse_parameter_list()?;
        self.consume(TokenKind::RightParen, "Expected ')' after parameters")?;

        let return_type = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let throws = if self.match_token(&[TokenKind::Throws]) {
            let mut error_types = Vec::new();

            // Check if there's a left parenthesis - if not, it's a single type without parens
            if self.check(&TokenKind::LeftParen) {
                self.consume(TokenKind::LeftParen, "Expected '(' after 'throws'")?;
                if !self.check(&TokenKind::RightParen) {
                    loop {
                        error_types.push(self.parse_type()?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RightParen, "Expected ')' after throws types")?;
            } else {
                // Single error type without parentheses
                error_types.push(self.parse_type()?);
            }
            Some(error_types)
        } else {
            None
        };

        // Support both brace-style { } and Lua-style ... end
        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after function body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after function body")?;
        }
        let end_span = self.current_span();

        Ok(Statement::Function(FunctionDeclaration {
            name,
            type_parameters,
            parameters,
            return_type,
            throws,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::If, "Expected 'if'")?;

        let condition = self.parse_expression()?;
        self.consume(TokenKind::Then, "Expected 'then' after if condition")?;

        let then_block = self.parse_block()?;

        let mut else_ifs = Vec::new();
        while self.match_token(&[TokenKind::Elseif]) {
            let elseif_start = self.current_span();
            let elseif_condition = self.parse_expression()?;
            self.consume(TokenKind::Then, "Expected 'then' after elseif condition")?;
            let elseif_block = self.parse_block()?;
            let elseif_end = elseif_block.span;

            else_ifs.push(ElseIf {
                condition: elseif_condition,
                block: elseif_block,
                span: elseif_start.combine(&elseif_end),
            });
        }

        let else_block = if self.match_token(&[TokenKind::Else]) {
            Some(self.parse_block()?)
        } else {
            None
        };

        self.consume(TokenKind::End, "Expected 'end' after if statement")?;
        let end_span = self.current_span();

        Ok(Statement::If(IfStatement {
            condition,
            then_block,
            else_ifs,
            else_block,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_while_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::While, "Expected 'while'")?;

        let condition = self.parse_expression()?;
        self.consume(TokenKind::Do, "Expected 'do' after while condition")?;

        let body = self.parse_block()?;
        self.consume(TokenKind::End, "Expected 'end' after while body")?;
        let end_span = self.current_span();

        Ok(Statement::While(WhileStatement {
            condition,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_repeat_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Repeat, "Expected 'repeat'")?;

        let body = self.parse_block()?;
        self.consume(TokenKind::Until, "Expected 'until' after repeat body")?;

        let until = self.parse_expression()?;
        let end_span = until.span;

        Ok(Statement::Repeat(RepeatStatement {
            body,
            until,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_for_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::For, "Expected 'for'")?;

        // Peek ahead to determine if this is numeric or generic for

        // Try parsing as numeric for: for i = start, end, step do
        let first_var = self.parse_identifier()?;

        if self.match_token(&[TokenKind::Equal]) {
            // Numeric for
            let start = self.parse_expression()?;
            self.consume(TokenKind::Comma, "Expected ',' after for start value")?;
            let end = self.parse_expression()?;

            let step = if self.match_token(&[TokenKind::Comma]) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            self.consume(TokenKind::Do, "Expected 'do' after for range")?;
            let body = self.parse_block()?;
            self.consume(TokenKind::End, "Expected 'end' after for body")?;
            let end_span = self.current_span();

            Ok(Statement::For(Box::new(ForStatement::Numeric(Box::new(
                ForNumeric {
                    variable: first_var,
                    start,
                    end,
                    step,
                    body,
                    span: start_span.combine(&end_span),
                },
            )))))
        } else {
            // Generic for: for k, v in iterator do
            let mut variables = vec![first_var];

            while self.match_token(&[TokenKind::Comma]) {
                variables.push(self.parse_identifier()?);
            }

            self.consume(TokenKind::In, "Expected 'in' in for loop")?;

            let mut iterators = vec![self.parse_expression()?];
            while self.match_token(&[TokenKind::Comma]) {
                iterators.push(self.parse_expression()?);
            }

            self.consume(TokenKind::Do, "Expected 'do' after for iterators")?;
            let body = self.parse_block()?;
            self.consume(TokenKind::End, "Expected 'end' after for body")?;
            let end_span = self.current_span();

            Ok(Statement::For(Box::new(ForStatement::Generic(
                ForGeneric {
                    variables,
                    iterators,
                    body,
                    span: start_span.combine(&end_span),
                },
            ))))
        }
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Return, "Expected 'return'")?;

        let mut values = Vec::new();

        // Return can have no values
        if !matches!(
            &self.current().kind,
            TokenKind::End
                | TokenKind::Else
                | TokenKind::Elseif
                | TokenKind::Until
                | TokenKind::Eof
        ) {
            values.push(self.parse_expression()?);

            while self.match_token(&[TokenKind::Comma]) {
                values.push(self.parse_expression()?);
            }
        }

        let end_span = if !values.is_empty() {
            values.last().unwrap().span
        } else {
            start_span
        };

        Ok(Statement::Return(ReturnStatement {
            values,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_interface_declaration(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::Interface, "Expected 'interface'")?;
        let interface = self.parse_interface_inner()?;
        Ok(Statement::Interface(interface))
    }

    fn parse_interface_inner(&mut self) -> Result<InterfaceDeclaration, ParserError> {
        let start_span = self.current_span();
        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        let mut extends = Vec::new();
        if self.match_token(&[TokenKind::Extends]) {
            extends.push(self.parse_type()?);
            while self.match_token(&[TokenKind::Comma]) {
                extends.push(self.parse_type()?);
            }
        }

        self.consume(TokenKind::LeftBrace, "Expected '{' after interface header")?;

        let members = self.parse_interface_members()?;

        self.consume(TokenKind::RightBrace, "Expected '}' after interface body")?;
        let end_span = self.current_span();

        Ok(InterfaceDeclaration {
            name,
            type_parameters,
            extends,
            members,
            span: start_span.combine(&end_span),
        })
    }

    fn parse_interface_members(&mut self) -> Result<Vec<InterfaceMember>, ParserError> {
        let mut members = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            // Check for index signature: [key: string]: Type
            if self.check(&TokenKind::LeftBracket) {
                members.push(InterfaceMember::Index(self.parse_index_signature()?));
            } else {
                let is_readonly = self.match_token(&[TokenKind::Readonly]);
                let name = self.parse_identifier()?;

                // Check if it's a method or property
                if self.check(&TokenKind::LeftParen) || self.check(&TokenKind::LessThan) {
                    // Method signature
                    let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
                        Some(self.parse_type_parameters()?)
                    } else {
                        None
                    };

                    self.consume(TokenKind::LeftParen, "Expected '('")?;
                    let parameters = self.parse_parameter_list()?;
                    self.consume(TokenKind::RightParen, "Expected ')'")?;

                    self.consume(TokenKind::Colon, "Expected ':' after method parameters")?;
                    let return_type = self.parse_type()?;
                    let span = name.span.combine(&return_type.span);

                    members.push(InterfaceMember::Method(MethodSignature {
                        name,
                        type_parameters,
                        parameters,
                        return_type,
                        span,
                    }));
                } else {
                    // Property signature
                    let is_optional = self.match_token(&[TokenKind::Question]);
                    self.consume(TokenKind::Colon, "Expected ':' after property name")?;
                    let type_annotation = self.parse_type()?;
                    let span = name.span.combine(&type_annotation.span);

                    members.push(InterfaceMember::Property(PropertySignature {
                        is_readonly,
                        name,
                        is_optional,
                        type_annotation,
                        span,
                    }));
                }

                // Optional comma or semicolon
                self.match_token(&[TokenKind::Comma, TokenKind::Semicolon]);
            }
        }

        Ok(members)
    }

    pub(super) fn parse_index_signature(&mut self) -> Result<IndexSignature, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::LeftBracket, "Expected '['")?;

        let key_name = self.parse_identifier()?;
        self.consume(TokenKind::Colon, "Expected ':' after index key name")?;

        let key_type = match &self.current().kind {
            TokenKind::Identifier(s) if self.resolve(*s) == "string" => {
                self.advance();
                IndexKeyType::String
            }
            TokenKind::Identifier(s) if self.resolve(*s) == "number" => {
                self.advance();
                IndexKeyType::Number
            }
            _ => {
                return Err(ParserError {
                    message: "Index signature key must be 'string' or 'number'".to_string(),
                    span: self.current_span(),
                })
            }
        };

        self.consume(TokenKind::RightBracket, "Expected ']'")?;
        self.consume(TokenKind::Colon, "Expected ':' after index signature key")?;

        let value_type = self.parse_type()?;
        let end_span = value_type.span;

        // Optional comma or semicolon
        self.match_token(&[TokenKind::Comma, TokenKind::Semicolon]);

        Ok(IndexSignature {
            key_name,
            key_type,
            value_type,
            span: start_span.combine(&end_span),
        })
    }

    fn parse_type_alias_declaration(&mut self) -> Result<Statement, ParserError> {
        self.consume(TokenKind::Type, "Expected 'type'")?;
        let type_alias = self.parse_type_alias_inner()?;
        Ok(Statement::TypeAlias(type_alias))
    }

    fn parse_type_alias_inner(&mut self) -> Result<TypeAliasDeclaration, ParserError> {
        let start_span = self.current_span();
        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        self.consume(TokenKind::Equal, "Expected '=' in type alias")?;

        let type_annotation = self.parse_type()?;
        let end_span = type_annotation.span;

        Ok(TypeAliasDeclaration {
            name,
            type_parameters,
            type_annotation,
            span: start_span.combine(&end_span),
        })
    }

    fn parse_enum_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Enum, "Expected 'enum'")?;

        let name = self.parse_identifier()?;

        self.consume(TokenKind::LeftBrace, "Expected '{' after enum name")?;

        let mut members = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let member_start = self.current_span();
            let member_name = self.parse_identifier()?;

            let value = if self.match_token(&[TokenKind::Equal]) {
                match &self.current().kind {
                    TokenKind::Number(s) => {
                        let val = s.parse::<f64>().map_err(|_| ParserError {
                            message: "Invalid number in enum value".to_string(),
                            span: self.current_span(),
                        })?;
                        self.advance();
                        Some(EnumValue::Number(val))
                    }
                    TokenKind::String(s) => {
                        let val = s.clone();
                        self.advance();
                        Some(EnumValue::String(val))
                    }
                    _ => {
                        return Err(ParserError {
                            message: "Enum value must be a number or string".to_string(),
                            span: self.current_span(),
                        })
                    }
                }
            } else {
                None
            };

            let member_end = self.current_span();

            members.push(EnumMember {
                name: member_name,
                value,
                span: member_start.combine(&member_end),
            });

            if !self.check(&TokenKind::RightBrace) {
                self.consume(TokenKind::Comma, "Expected ',' between enum members")?;
            }
        }

        self.consume(TokenKind::RightBrace, "Expected '}' after enum body")?;
        let end_span = self.current_span();

        Ok(Statement::Enum(EnumDeclaration {
            name,
            members,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_import_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Import, "Expected 'import'")?;

        // Check for "import type { ... }"
        let is_type_only = if self.check(&TokenKind::Type) {
            self.advance(); // consume 'type'
            true
        } else {
            false
        };

        // Parse import clause
        let clause = if self.match_token(&[TokenKind::Star]) {
            // import * as name from "source"
            self.consume(TokenKind::As, "Expected 'as' after '*'")?;
            let name = self.parse_identifier()?;
            ImportClause::Namespace(name)
        } else if self.check(&TokenKind::LeftBrace) {
            // import { a, b as c } from "source" OR import type { a, b } from "source"
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
            let specifiers = self.parse_import_specifiers()?;
            self.consume(TokenKind::RightBrace, "Expected '}'")?;
            if is_type_only {
                ImportClause::TypeOnly(specifiers)
            } else {
                ImportClause::Named(specifiers)
            }
        } else {
            // import name from "source"
            if is_type_only {
                return Err(ParserError {
                    message: "Type-only imports must use named import syntax: import type { Name } from '...'".to_string(),
                    span: self.current_span(),
                });
            }
            let name = self.parse_identifier()?;
            ImportClause::Default(name)
        };

        self.consume(TokenKind::From, "Expected 'from' in import")?;

        let source = match &self.current().kind {
            TokenKind::String(s) => {
                let src = s.clone();
                self.advance();
                src
            }
            _ => {
                return Err(ParserError {
                    message: "Expected string literal for import source".to_string(),
                    span: self.current_span(),
                })
            }
        };

        let end_span = self.current_span();

        Ok(Statement::Import(ImportDeclaration {
            clause,
            source,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_import_specifiers(&mut self) -> Result<Vec<ImportSpecifier>, ParserError> {
        let mut specifiers = Vec::new();

        loop {
            let imported = self.parse_identifier()?;
            let local = if self.match_token(&[TokenKind::As]) {
                Some(self.parse_identifier()?)
            } else {
                None
            };

            let span = if let Some(ref l) = local {
                imported.span.combine(&l.span)
            } else {
                imported.span
            };

            specifiers.push(ImportSpecifier {
                imported,
                local,
                span,
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(specifiers)
    }

    fn parse_export_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Export, "Expected 'export'")?;

        let is_default = match &self.current().kind {
            TokenKind::Identifier(s) if self.resolve(*s) == "default" => {
                self.advance();
                true
            }
            _ => false,
        };

        let kind = if is_default {
            // export default expression
            let expr = self.parse_expression()?;
            ExportKind::Default(expr)
        } else if self.check(&TokenKind::LeftBrace) {
            // export { a, b as c } [from './source']
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
            let specifiers = self.parse_export_specifiers()?;
            self.consume(TokenKind::RightBrace, "Expected '}'")?;

            // Check for 'from' clause (re-export)
            let source = if self.match_token(&[TokenKind::From]) {
                match &self.current().kind {
                    TokenKind::String(s) => {
                        let source_string = s.clone();
                        self.advance();
                        Some(source_string)
                    }
                    _ => {
                        return Err(ParserError {
                            message: "Expected string literal after 'from'".to_string(),
                            span: self.current_span(),
                        });
                    }
                }
            } else {
                None
            };

            ExportKind::Named { specifiers, source }
        } else {
            // export declaration
            let decl = self.parse_statement()?;
            ExportKind::Declaration(Box::new(decl))
        };

        let end_span = self.current_span();

        Ok(Statement::Export(ExportDeclaration {
            kind,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_export_specifiers(&mut self) -> Result<Vec<ExportSpecifier>, ParserError> {
        let mut specifiers = Vec::new();

        loop {
            let local = self.parse_identifier()?;
            let exported = if self.match_token(&[TokenKind::As]) {
                Some(self.parse_identifier()?)
            } else {
                None
            };

            let span = if let Some(ref e) = exported {
                local.span.combine(&e.span)
            } else {
                local.span
            };

            specifiers.push(ExportSpecifier {
                local,
                exported,
                span,
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(specifiers)
    }

    fn parse_declare_statement(&mut self) -> Result<Statement, ParserError> {
        let _start_span = self.current_span();
        self.consume(TokenKind::Declare, "Expected 'declare'")?;

        match &self.current().kind {
            TokenKind::Function => self.parse_declare_function(),
            TokenKind::Const => self.parse_declare_const(),
            TokenKind::Namespace => self.parse_declare_namespace(),
            TokenKind::Type => self.parse_declare_type(),
            TokenKind::Interface => self.parse_declare_interface(),
            _ => Err(ParserError {
                message: "Expected 'function', 'const', 'namespace', 'type', or 'interface' after 'declare'".to_string(),
                span: self.current_span(),
            }),
        }
    }

    fn parse_declare_function(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Function, "Expected 'function'")?;

        // Allow keywords as function names in declarations (e.g., type, string, etc.)
        let name = self.parse_identifier_or_keyword()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        self.consume(TokenKind::LeftParen, "Expected '(' after function name")?;
        let parameters = self.parse_parameter_list()?;
        self.consume(TokenKind::RightParen, "Expected ')' after parameters")?;

        let return_type = if self.match_token(&[TokenKind::Colon]) {
            self.parse_type()?
        } else {
            Type::new(
                TypeKind::Primitive(PrimitiveType::Void),
                self.current_span(),
            )
        };

        let throws = if self.match_token(&[TokenKind::Throws]) {
            let mut error_types = Vec::new();

            // Check if there's a left parenthesis - if not, it's a single type without parens
            if self.check(&TokenKind::LeftParen) {
                self.consume(TokenKind::LeftParen, "Expected '(' after 'throws'")?;
                if !self.check(&TokenKind::RightParen) {
                    loop {
                        error_types.push(self.parse_type()?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RightParen, "Expected ')' after throws types")?;
            } else {
                // Single error type without parentheses
                error_types.push(self.parse_type()?);
            }
            Some(error_types)
        } else {
            None
        };

        let end_span = self.current_span();

        Ok(Statement::DeclareFunction(DeclareFunctionStatement {
            name,
            type_parameters,
            parameters,
            return_type,
            throws,
            is_export: false,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_declare_const(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Const, "Expected 'const'")?;

        let name = self.parse_identifier()?;

        self.consume(TokenKind::Colon, "Expected ':' after const name")?;
        let type_annotation = self.parse_type()?;

        let end_span = self.current_span();

        Ok(Statement::DeclareConst(DeclareConstStatement {
            name,
            type_annotation,
            is_export: false,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_declare_namespace(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Namespace, "Expected 'namespace'")?;

        let name = self.parse_identifier()?;

        self.consume(TokenKind::LeftBrace, "Expected '{' after namespace name")?;

        let mut members = Vec::new();
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            // Check for 'export' keyword inside namespace
            let is_export = self.match_token(&[TokenKind::Export]);

            let member = match &self.current().kind {
                TokenKind::Function => {
                    let mut func_stmt = self.parse_declare_function()?;
                    // Mark as export if needed
                    if let Statement::DeclareFunction(ref mut func) = func_stmt {
                        func.is_export = is_export;
                    }
                    func_stmt
                }
                TokenKind::Const => {
                    let mut const_stmt = self.parse_declare_const()?;
                    if let Statement::DeclareConst(ref mut const_decl) = const_stmt {
                        const_decl.is_export = is_export;
                    }
                    const_stmt
                }
                _ => {
                    return Err(ParserError {
                        message: "Expected 'function' or 'const' in namespace".to_string(),
                        span: self.current_span(),
                    });
                }
            };

            members.push(member);
        }

        self.consume(TokenKind::RightBrace, "Expected '}' after namespace body")?;

        let end_span = self.current_span();

        Ok(Statement::DeclareNamespace(DeclareNamespaceStatement {
            name,
            members,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_declare_type(&mut self) -> Result<Statement, ParserError> {
        // declare type is the same as type alias
        self.consume(TokenKind::Type, "Expected 'type'")?;
        let type_alias = self.parse_type_alias_inner()?;
        Ok(Statement::DeclareType(type_alias))
    }

    fn parse_declare_interface(&mut self) -> Result<Statement, ParserError> {
        // declare interface is the same as interface
        self.consume(TokenKind::Interface, "Expected 'interface'")?;
        let interface = self.parse_interface_inner()?;
        Ok(Statement::DeclareInterface(interface))
    }

    fn parse_class_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();

        // Parse decorators
        let decorators = self.parse_decorators()?;

        let mut is_abstract = false;
        let mut is_final = false;

        // Allow abstract and final in any order
        loop {
            if self.match_token(&[TokenKind::Abstract]) {
                is_abstract = true;
            } else if self.match_token(&[TokenKind::Final]) {
                is_final = true;
            } else {
                break;
            }
        }

        self.consume(TokenKind::Class, "Expected 'class'")?;

        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        // Parse primary constructor parameters: class Name(params)
        let primary_constructor = if self.match_token(&[TokenKind::LeftParen]) {
            let params = self.parse_primary_constructor_parameters()?;
            self.consume(
                TokenKind::RightParen,
                "Expected ')' after primary constructor parameters",
            )?;
            Some(params)
        } else {
            None
        };

        // Parse extends clause with optional parent constructor arguments
        let (extends, parent_constructor_args) = if self.match_token(&[TokenKind::Extends]) {
            let parent_type = self.parse_type()?;

            // Parse parent constructor arguments: extends Parent(arg1, arg2)
            let parent_args = if self.match_token(&[TokenKind::LeftParen]) {
                let mut args = Vec::new();
                if !self.check(&TokenKind::RightParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(
                    TokenKind::RightParen,
                    "Expected ')' after parent constructor arguments",
                )?;
                Some(args)
            } else {
                None
            };

            (Some(parent_type), parent_args)
        } else {
            (None, None)
        };

        let mut implements = Vec::new();
        if self.match_token(&[TokenKind::Implements]) {
            implements.push(self.parse_type()?);
            while self.match_token(&[TokenKind::Comma]) {
                implements.push(self.parse_type()?);
            }
        }

        // Support both Lua-style (no braces, end with 'end') and TypeScript-style (braces)
        let use_braces = self.check(&TokenKind::LeftBrace);

        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{' after class header")?;
        }

        let mut members = Vec::new();
        if use_braces {
            while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
                members.push(self.parse_class_member()?);
            }
            self.consume(TokenKind::RightBrace, "Expected '}' after class body")?;
        } else {
            while !self.check(&TokenKind::End) && !self.is_at_end() {
                members.push(self.parse_class_member()?);
            }
            self.consume(TokenKind::End, "Expected 'end' after class body")?;
        }

        // Validate: cannot have both primary constructor and parameterized constructor
        if primary_constructor.is_some() {
            let has_parameterized_constructor = members
                .iter()
                .any(|m| matches!(m, ClassMember::Constructor(c) if !c.parameters.is_empty()));

            if has_parameterized_constructor {
                return Err(ParserError {
                    message: "Cannot have both a primary constructor and a parameterized constructor in the same class".to_string(),
                    span: start_span,
                });
            }
        }

        let end_span = self.current_span();

        Ok(Statement::Class(ClassDeclaration {
            decorators,
            is_abstract,
            is_final,
            name,
            type_parameters,
            primary_constructor,
            extends,
            parent_constructor_args,
            implements,
            members,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, ParserError> {
        let decorators = self.parse_decorators()?;

        // Check for access modifiers
        let access = if self.match_token(&[TokenKind::Public]) {
            Some(AccessModifier::Public)
        } else if self.match_token(&[TokenKind::Private]) {
            Some(AccessModifier::Private)
        } else if self.match_token(&[TokenKind::Protected]) {
            Some(AccessModifier::Protected)
        } else {
            None
        };

        let is_static = self.match_token(&[TokenKind::Static]);
        let mut is_abstract = false;
        let mut is_final = false;

        // Allow abstract and final in any order
        loop {
            if self.match_token(&[TokenKind::Abstract]) {
                is_abstract = true;
            } else if self.match_token(&[TokenKind::Final]) {
                is_final = true;
            } else {
                break;
            }
        }
        let is_override = self.match_token(&[TokenKind::Override]);
        let is_readonly = self.match_token(&[TokenKind::Readonly]);

        // Check for getter/setter
        if self.check(&TokenKind::Get) {
            return self.parse_getter(decorators, access, is_static);
        }
        if self.check(&TokenKind::Set) {
            return self.parse_setter(decorators, access, is_static);
        }

        // Check for constructor
        if self.check(&TokenKind::Constructor) {
            return self.parse_constructor(decorators);
        }

        // Check for operator
        if self.check(&TokenKind::Operator) {
            return self.parse_operator(decorators, access);
        }

        let start_span = self.current_span();
        let name = self.parse_identifier()?;

        // Property or Method
        if self.check(&TokenKind::Colon) {
            // Property: name: Type = value
            self.advance(); // consume ':'
            let type_annotation = self.parse_type()?;

            let initializer = if self.match_token(&[TokenKind::Equal]) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            // Optional semicolon after property
            self.match_token(&[TokenKind::Semicolon]);

            let end_span = self.current_span();

            Ok(ClassMember::Property(PropertyDeclaration {
                decorators,
                access,
                is_static,
                is_readonly,
                name,
                type_annotation,
                initializer,
                span: start_span.combine(&end_span),
            }))
        } else if self.check(&TokenKind::LeftParen) {
            // Method: name(params): ReturnType { body }
            let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
                Some(self.parse_type_parameters()?)
            } else {
                None
            };

            self.consume(TokenKind::LeftParen, "Expected '('")?;
            let parameters = self.parse_parameter_list()?;
            self.consume(TokenKind::RightParen, "Expected ')'")?;

            let return_type = if self.match_token(&[TokenKind::Colon]) {
                Some(self.parse_type()?)
            } else {
                None
            };

            let body = if is_abstract {
                // Abstract methods have no body, just a semicolon
                self.match_token(&[TokenKind::Semicolon]); // Optional semicolon
                None
            } else {
                // Support both { } and Lua-style ... end
                let use_braces = self.check(&TokenKind::LeftBrace);
                if use_braces {
                    self.consume(TokenKind::LeftBrace, "Expected '{'")?;
                }
                let block = self.parse_block()?;
                if use_braces {
                    self.consume(TokenKind::RightBrace, "Expected '}' after method body")?;
                } else {
                    self.consume(TokenKind::End, "Expected 'end' after method body")?;
                }
                Some(block)
            };

            let end_span = self.current_span();

            Ok(ClassMember::Method(MethodDeclaration {
                decorators,
                access,
                is_static,
                is_abstract,
                is_override,
                is_final,
                name,
                type_parameters,
                parameters,
                return_type,
                body,
                span: start_span.combine(&end_span),
            }))
        } else {
            Err(ParserError {
                message: "Expected ':' for property or '(' for method".into(),
                span: self.current_span(),
            })
        }
    }

    fn parse_constructor(
        &mut self,
        decorators: Vec<Decorator>,
    ) -> Result<ClassMember, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Constructor, "Expected 'constructor'")?;
        self.consume(TokenKind::LeftParen, "Expected '('")?;
        let parameters = self.parse_parameter_list()?;
        self.consume(TokenKind::RightParen, "Expected ')'")?;

        // Support both { } and Lua-style ... end
        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after constructor body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after constructor body")?;
        }
        let end_span = self.current_span();

        Ok(ClassMember::Constructor(ConstructorDeclaration {
            decorators,
            parameters,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_getter(
        &mut self,
        decorators: Vec<Decorator>,
        access: Option<AccessModifier>,
        is_static: bool,
    ) -> Result<ClassMember, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Get, "Expected 'get'")?;
        let name = self.parse_identifier()?;
        self.consume(TokenKind::LeftParen, "Expected '('")?;
        self.consume(TokenKind::RightParen, "Expected ')'")?;
        self.consume(TokenKind::Colon, "Expected ':' for getter return type")?;
        let return_type = self.parse_type()?;

        // Support both { } and Lua-style ... end
        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after getter body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after getter body")?;
        }
        let end_span = self.current_span();

        Ok(ClassMember::Getter(GetterDeclaration {
            decorators,
            access,
            is_static,
            name,
            return_type,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_setter(
        &mut self,
        decorators: Vec<Decorator>,
        access: Option<AccessModifier>,
        is_static: bool,
    ) -> Result<ClassMember, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Set, "Expected 'set'")?;
        let name = self.parse_identifier()?;
        self.consume(TokenKind::LeftParen, "Expected '('")?;

        let param_start = self.current_span();
        let param_pattern = self.parse_pattern()?;
        self.consume(TokenKind::Colon, "Expected ':' for setter parameter type")?;
        let param_type = self.parse_type()?;

        self.consume(TokenKind::RightParen, "Expected ')'")?;

        // Support both { } and Lua-style ... end
        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after setter body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after setter body")?;
        }
        let end_span = self.current_span();

        let parameter = Parameter {
            pattern: param_pattern,
            type_annotation: Some(param_type),
            default: None,
            is_rest: false,
            is_optional: false,
            span: param_start,
        };

        Ok(ClassMember::Setter(SetterDeclaration {
            decorators,
            access,
            is_static,
            name,
            parameter,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_operator(
        &mut self,
        decorators: Vec<Decorator>,
        access: Option<AccessModifier>,
    ) -> Result<ClassMember, ParserError> {
        eprintln!("parse_operator: current token is {:?}", self.current().kind);
        let start_span = self.current_span();
        self.consume(TokenKind::Operator, "Expected 'operator'")?;

        let operator_kind = self.parse_operator_kind()?;

        let mut operator = operator_kind;

        // Check for unary minus: operator -() with empty parens
        // Use lookahead to avoid consuming ( before we know it's unary minus
        if operator == OperatorKind::Subtract
            && self.check(&TokenKind::LeftParen)
            && self.nth_token_kind(1) == Some(&TokenKind::RightParen)
        {
            self.advance(); // consume (
            self.advance(); // consume )
            operator = OperatorKind::UnaryMinus;
        }

        let mut parameters = Vec::new();

        if operator == OperatorKind::NewIndex {
            self.consume(TokenKind::LeftParen, "Expected '(' after operator")?;
            let params = self.parse_parameter_list()?;
            self.consume(TokenKind::RightParen, "Expected ')'")?;
            if params.len() != 2 {
                return Err(ParserError {
                    message: "operator []= requires exactly 2 parameters (index and value)".into(),
                    span: self.current_span(),
                });
            }
            parameters = params;
        } else if self.check(&TokenKind::LeftParen) {
            self.consume(TokenKind::LeftParen, "Expected '(' after operator")?;
            if !self.check(&TokenKind::RightParen) {
                let params = self.parse_parameter_list()?;
                self.consume(TokenKind::RightParen, "Expected ')'")?;
                parameters = params;
            } else {
                self.consume(TokenKind::RightParen, "Expected ')'")?;
            }
        }

        let return_type = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // Support both brace-style { } and Lua-style ... end
        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after operator body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after operator body")?;
        }

        // Debug: print next token
        // eprintln!("After operator: next token is {:?}", self.current().kind);
        let end_span = self.current_span();

        Ok(ClassMember::Operator(OperatorDeclaration {
            decorators,
            access,
            operator,
            parameters,
            return_type,
            body,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_operator_kind(&mut self) -> Result<OperatorKind, ParserError> {
        let op = match &self.current().kind {
            TokenKind::Plus => {
                self.advance();
                OperatorKind::Add
            }
            TokenKind::Minus => {
                self.advance();
                OperatorKind::Subtract
            }
            TokenKind::Star => {
                self.advance();
                OperatorKind::Multiply
            }
            TokenKind::Slash => {
                self.advance();
                OperatorKind::Divide
            }
            TokenKind::Percent => {
                self.advance();
                OperatorKind::Modulo
            }
            TokenKind::Caret => {
                self.advance();
                OperatorKind::Power
            }
            TokenKind::DotDot => {
                self.advance();
                OperatorKind::Concatenate
            }
            TokenKind::SlashSlash => {
                self.advance();
                OperatorKind::FloorDivide
            }
            TokenKind::EqualEqual => {
                self.advance();
                OperatorKind::Equal
            }
            TokenKind::BangEqual | TokenKind::TildeEqual => {
                self.advance();
                OperatorKind::NotEqual
            }
            TokenKind::LessThan => {
                self.advance();
                OperatorKind::LessThan
            }
            TokenKind::LessEqual => {
                self.advance();
                OperatorKind::LessThanOrEqual
            }
            TokenKind::GreaterThan => {
                self.advance();
                OperatorKind::GreaterThan
            }
            TokenKind::GreaterEqual => {
                self.advance();
                OperatorKind::GreaterThanOrEqual
            }
            TokenKind::Ampersand => {
                self.advance();
                OperatorKind::BitwiseAnd
            }
            TokenKind::Pipe => {
                self.advance();
                OperatorKind::BitwiseOr
            }
            TokenKind::Tilde => {
                self.advance();
                OperatorKind::BitwiseXor
            }
            TokenKind::LessLess => {
                self.advance();
                OperatorKind::ShiftLeft
            }
            TokenKind::GreaterGreater => {
                self.advance();
                OperatorKind::ShiftRight
            }
            TokenKind::LeftBracket => {
                self.advance();
                if self.check(&TokenKind::RightBracket)
                    && self.nth_token_kind(1) == Some(&TokenKind::Equal)
                {
                    self.advance();
                    self.advance();
                    OperatorKind::NewIndex
                } else if self.match_token(&[TokenKind::RightBracket]) {
                    OperatorKind::Index
                } else {
                    return Err(ParserError {
                        message: "Expected ']' or '=' after '[' in operator definition".into(),
                        span: self.current_span(),
                    });
                }
            }
            TokenKind::LeftParen => {
                self.advance();
                self.consume(
                    TokenKind::RightParen,
                    "Expected ')' after '(' in operator()",
                )?;
                OperatorKind::Call
            }
            TokenKind::Hash => {
                self.advance();
                OperatorKind::Length
            }
            _ => {
                return Err(ParserError {
                    message: "Invalid operator symbol".to_string(),
                    span: self.current_span(),
                });
            }
        };
        Ok(op)
    }

    fn parse_throw_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Throw, "Expected 'throw'")?;

        let expression = self.parse_expression()?;
        let end_span = expression.span;

        Ok(Statement::Throw(ThrowStatement {
            expression,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_rethrow_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Rethrow, "Expected 'rethrow'")?;

        Ok(Statement::Rethrow(start_span))
    }

    fn parse_try_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Try, "Expected 'try'")?;

        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let try_block = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after try block")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after try block")?;
        }

        let mut catch_clauses = Vec::new();
        while self.match_token(&[TokenKind::Catch]) {
            catch_clauses.push(self.parse_catch_clause()?);
        }

        let finally_block = if self.match_token(&[TokenKind::Finally]) {
            let use_braces = self.check(&TokenKind::LeftBrace);
            if use_braces {
                self.consume(TokenKind::LeftBrace, "Expected '{'")?;
            }
            let finally = self.parse_block()?;
            if use_braces {
                self.consume(TokenKind::RightBrace, "Expected '}' after finally block")?;
            } else {
                self.consume(TokenKind::End, "Expected 'end' after finally block")?;
            }
            Some(finally)
        } else {
            None
        };

        let end_span = if let Some(ref last_catch) = catch_clauses.last() {
            last_catch.span
        } else if let Some(ref fin) = finally_block {
            fin.span
        } else {
            try_block.span
        };

        Ok(Statement::Try(TryStatement {
            try_block,
            catch_clauses,
            finally_block,
            span: start_span.combine(&end_span),
        }))
    }

    fn parse_catch_clause(&mut self) -> Result<CatchClause, ParserError> {
        let start_span = self.current_span();

        self.consume(TokenKind::LeftParen, "Expected '(' after 'catch'")?;

        let variable = self.parse_identifier()?;
        let span_after_var = self.current_span();

        let pattern = if self.match_token(&[TokenKind::Colon]) {
            let mut type_annotations = Vec::new();
            type_annotations.push(self.parse_type()?);

            while self.match_token(&[TokenKind::Pipe]) {
                type_annotations.push(self.parse_type()?);
            }

            if type_annotations.len() == 1 {
                let type_annotation = type_annotations.into_iter().next().unwrap();
                let span = type_annotation.span;
                CatchPattern::Typed {
                    variable,
                    type_annotation,
                    span: start_span.combine(&span),
                }
            } else {
                CatchPattern::MultiTyped {
                    variable,
                    type_annotations,
                    span: start_span.combine(&span_after_var),
                }
            }
        } else {
            CatchPattern::Untyped {
                variable,
                span: start_span.combine(&span_after_var),
            }
        };

        self.consume(TokenKind::RightParen, "Expected ')' after catch parameter")?;

        let use_braces = self.check(&TokenKind::LeftBrace);
        if use_braces {
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
        }
        let body = self.parse_block()?;
        if use_braces {
            self.consume(TokenKind::RightBrace, "Expected '}' after catch body")?;
        } else {
            self.consume(TokenKind::End, "Expected 'end' after catch body")?;
        }

        let end_span = body.span;

        Ok(CatchClause {
            pattern,
            body,
            span: start_span.combine(&end_span),
        })
    }

    // Helper methods (pub so other parser modules can use them)

    fn parse_decorators(&mut self) -> Result<Vec<Decorator>, ParserError> {
        let mut decorators = Vec::new();

        while self.check(&TokenKind::At) {
            decorators.push(self.parse_decorator()?);
        }

        Ok(decorators)
    }

    fn parse_decorator(&mut self) -> Result<Decorator, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::At, "Expected '@'")?;

        let expression = self.parse_decorator_expression()?;
        let end_span = self.current_span();

        Ok(Decorator {
            expression,
            span: start_span.combine(&end_span),
        })
    }

    fn parse_decorator_expression(&mut self) -> Result<DecoratorExpression, ParserError> {
        let start_span = self.current_span();
        // Allow keywords as decorator names (e.g., @readonly, @sealed)
        let name = self.parse_identifier_or_keyword()?;

        let mut expr = DecoratorExpression::Identifier(name);

        loop {
            match &self.current().kind {
                TokenKind::Dot => {
                    self.advance();
                    // Allow keywords as property names in decorators
                    let property = self.parse_identifier_or_keyword()?;
                    let span = start_span.combine(&property.span);
                    expr = DecoratorExpression::Member {
                        object: Box::new(expr),
                        property,
                        span,
                    };
                }
                TokenKind::LeftParen => {
                    self.advance();
                    let mut arguments = Vec::new();

                    if !self.check(&TokenKind::RightParen) {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }

                    let end_span = self.current_span();
                    self.consume(
                        TokenKind::RightParen,
                        "Expected ')' after decorator arguments",
                    )?;

                    let span = start_span.combine(&end_span);
                    expr = DecoratorExpression::Call {
                        callee: Box::new(expr),
                        arguments,
                        span,
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    pub(super) fn parse_identifier(&mut self) -> Result<Ident, ParserError> {
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let span = self.current_span();
                let ident = Spanned::new(*name, span);
                self.advance();
                Ok(ident)
            }
            _ => Err(ParserError {
                message: format!("Expected identifier, got {:?}", self.current().kind),
                span: self.current_span(),
            }),
        }
    }

    // Parse identifier or keyword as identifier (for contexts like declare function type(...))
    fn parse_identifier_or_keyword(&mut self) -> Result<Ident, ParserError> {
        let span = self.current_span();
        let name = match &self.current().kind {
            TokenKind::Identifier(name) => *name,
            // Allow keywords as identifiers in certain contexts
            kind if kind.is_keyword() => {
                // Get the keyword string representation and intern it
                match kind.to_keyword_str() {
                    Some(s) => self.interner.intern(s),
                    None => {
                        return Err(ParserError {
                            message: format!(
                                "Internal error: keyword {:?} missing string representation",
                                kind
                            ),
                            span,
                        });
                    }
                }
            }
            _ => {
                return Err(ParserError {
                    message: format!(
                        "Expected identifier or keyword, got {:?}",
                        self.current().kind
                    ),
                    span,
                });
            }
        };

        self.advance();
        Ok(Spanned::new(name, span))
    }

    pub(super) fn parse_type_parameters(&mut self) -> Result<Vec<TypeParameter>, ParserError> {
        let mut params = Vec::new();

        loop {
            let param_start = self.current_span();
            let name = self.parse_identifier()?;

            let constraint = if self.match_token(&[TokenKind::Extends]) {
                Some(Box::new(self.parse_type()?))
            } else {
                None
            };

            let default = if self.match_token(&[TokenKind::Equal]) {
                Some(Box::new(self.parse_type()?))
            } else {
                None
            };

            let param_end = self.current_span();

            params.push(TypeParameter {
                name,
                constraint,
                default,
                span: param_start.combine(&param_end),
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        self.consume(TokenKind::GreaterThan, "Expected '>' after type parameters")?;

        Ok(params)
    }

    pub(super) fn parse_parameter_list(&mut self) -> Result<Vec<Parameter>, ParserError> {
        let mut params = Vec::new();

        if self.check(&TokenKind::RightParen) {
            return Ok(params);
        }

        loop {
            let param_start = self.current_span();
            let is_rest = self.match_token(&[TokenKind::DotDotDot]);

            let pattern = self.parse_pattern()?;

            // Check for optional marker (?)
            let is_optional = self.match_token(&[TokenKind::Question]);

            let type_annotation = if self.match_token(&[TokenKind::Colon]) {
                Some(self.parse_type()?)
            } else {
                None
            };

            let default = if self.match_token(&[TokenKind::Equal]) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            let param_end = self.current_span();

            params.push(Parameter {
                pattern,
                type_annotation,
                default,
                is_rest,
                is_optional,
                span: param_start.combine(&param_end),
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(params)
    }

    /// Parse primary constructor parameters with access modifiers and readonly
    /// Example: (public x: number, private readonly y: number = 0)
    fn parse_primary_constructor_parameters(
        &mut self,
    ) -> Result<Vec<ConstructorParameter>, ParserError> {
        use crate::ast::statement::ConstructorParameter;

        let mut params = Vec::new();

        if self.check(&TokenKind::RightParen) {
            return Ok(params);
        }

        loop {
            let param_start = self.current_span();

            // Parse access modifier (public, private, protected)
            let access = if self.match_token(&[TokenKind::Public]) {
                Some(AccessModifier::Public)
            } else if self.match_token(&[TokenKind::Private]) {
                Some(AccessModifier::Private)
            } else if self.match_token(&[TokenKind::Protected]) {
                Some(AccessModifier::Protected)
            } else {
                None
            };

            // Parse readonly modifier
            let is_readonly = self.match_token(&[TokenKind::Readonly]);

            // Parse parameter name
            let name = self.parse_identifier()?;

            // Parse type annotation (required for primary constructor parameters)
            self.consume(
                TokenKind::Colon,
                "Expected ':' after parameter name in primary constructor",
            )?;
            let type_annotation = self.parse_type()?;

            // Parse optional default value
            let default = if self.match_token(&[TokenKind::Equal]) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            let param_end = self.current_span();

            params.push(ConstructorParameter {
                access,
                is_readonly,
                name,
                type_annotation,
                default,
                span: param_start.combine(&param_end),
            });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        Ok(params)
    }
}

// Trait for getting span from Statement
trait Spannable {
    fn span(&self) -> Span;
}

impl Spannable for Statement {
    fn span(&self) -> Span {
        match self {
            Statement::Variable(v) => v.span,
            Statement::Function(f) => f.span,
            Statement::Class(c) => c.span,
            Statement::Interface(i) => i.span,
            Statement::TypeAlias(t) => t.span,
            Statement::Enum(e) => e.span,
            Statement::Import(i) => i.span,
            Statement::Export(e) => e.span,
            Statement::If(i) => i.span,
            Statement::While(w) => w.span,
            Statement::For(f) => match f.as_ref() {
                ForStatement::Numeric(n) => n.span,
                ForStatement::Generic(g) => g.span,
            },
            Statement::Repeat(r) => r.span,
            Statement::Return(r) => r.span,
            Statement::Break(s) | Statement::Continue(s) => *s,
            Statement::Expression(e) => e.span,
            Statement::Block(b) => b.span,
            Statement::DeclareFunction(f) => f.span,
            Statement::DeclareNamespace(n) => n.span,
            Statement::DeclareType(t) => t.span,
            Statement::DeclareInterface(i) => i.span,
            Statement::DeclareConst(c) => c.span,
            Statement::Throw(t) => t.span,
            Statement::Try(t) => t.span,
            Statement::Rethrow(s) => *s,
        }
    }
}
