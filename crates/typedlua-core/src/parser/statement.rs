use super::{ExpressionParser, Parser, ParserError, PatternParser, TypeParser};
use crate::ast::statement::*;
use crate::ast::Ident;
use crate::ast::Spanned;
use crate::lexer::TokenKind;
use crate::span::Span;

pub trait StatementParser {
    fn parse_statement(&mut self) -> Result<Statement, ParserError>;
    fn parse_block(&mut self) -> Result<Block, ParserError>;
}

impl StatementParser for Parser {
    fn parse_statement(&mut self) -> Result<Statement, ParserError> {
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
            TokenKind::Class => self.parse_class_declaration(),
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
impl Parser {
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

        self.consume(
            TokenKind::Equal,
            "Expected '=' in variable declaration",
        )?;

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

        let body = self.parse_block()?;
        self.consume(TokenKind::End, "Expected 'end' after function body")?;
        let end_span = self.current_span();

        Ok(Statement::Function(FunctionDeclaration {
            name,
            type_parameters,
            parameters,
            return_type,
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

            Ok(Statement::For(ForStatement::Numeric(ForNumeric {
                variable: first_var,
                start,
                end,
                step,
                body,
                span: start_span.combine(&end_span),
            })))
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

            Ok(Statement::For(ForStatement::Generic(ForGeneric {
                variables,
                iterators,
                body,
                span: start_span.combine(&end_span),
            })))
        }
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();
        self.consume(TokenKind::Return, "Expected 'return'")?;

        let mut values = Vec::new();

        // Return can have no values
        if !matches!(
            &self.current().kind,
            TokenKind::End | TokenKind::Else | TokenKind::Elseif | TokenKind::Until | TokenKind::Eof
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
        let start_span = self.current_span();
        self.consume(TokenKind::Interface, "Expected 'interface'")?;

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

        Ok(Statement::Interface(InterfaceDeclaration {
            name,
            type_parameters,
            extends,
            members,
            span: start_span.combine(&end_span),
        }))
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
            TokenKind::Identifier(s) if s == "string" => {
                self.advance();
                IndexKeyType::String
            }
            TokenKind::Identifier(s) if s == "number" => {
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
        let start_span = self.current_span();
        self.consume(TokenKind::Type, "Expected 'type'")?;

        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        self.consume(TokenKind::Equal, "Expected '=' in type alias")?;

        let type_annotation = self.parse_type()?;
        let end_span = type_annotation.span;

        Ok(Statement::TypeAlias(TypeAliasDeclaration {
            name,
            type_parameters,
            type_annotation,
            span: start_span.combine(&end_span),
        }))
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

        // Parse import clause
        let clause = if self.match_token(&[TokenKind::Star]) {
            // import * as name from "source"
            self.consume(TokenKind::As, "Expected 'as' after '*'")?;
            let name = self.parse_identifier()?;
            ImportClause::Namespace(name)
        } else if self.check(&TokenKind::LeftBrace) {
            // import { a, b as c } from "source"
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
            let specifiers = self.parse_import_specifiers()?;
            self.consume(TokenKind::RightBrace, "Expected '}'")?;
            ImportClause::Named(specifiers)
        } else {
            // import name from "source"
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
            TokenKind::Identifier(s) if s == "default" => {
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
            // export { a, b as c }
            self.consume(TokenKind::LeftBrace, "Expected '{'")?;
            let specifiers = self.parse_export_specifiers()?;
            self.consume(TokenKind::RightBrace, "Expected '}'")?;
            ExportKind::Named(specifiers)
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

    fn parse_class_declaration(&mut self) -> Result<Statement, ParserError> {
        let start_span = self.current_span();

        // Parse decorators
        let decorators = Vec::new(); // TODO: Implement decorator parsing

        let is_abstract = self.match_token(&[TokenKind::Abstract]);

        self.consume(TokenKind::Class, "Expected 'class'")?;

        let name = self.parse_identifier()?;

        let type_parameters = if self.match_token(&[TokenKind::LessThan]) {
            Some(self.parse_type_parameters()?)
        } else {
            None
        };

        let extends = if self.match_token(&[TokenKind::Extends]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let mut implements = Vec::new();
        if self.match_token(&[TokenKind::Implements]) {
            implements.push(self.parse_type()?);
            while self.match_token(&[TokenKind::Comma]) {
                implements.push(self.parse_type()?);
            }
        }

        self.consume(TokenKind::LeftBrace, "Expected '{' after class header")?;

        let members = Vec::new(); // TODO: Implement class member parsing

        self.consume(TokenKind::RightBrace, "Expected '}' after class body")?;
        let end_span = self.current_span();

        Ok(Statement::Class(ClassDeclaration {
            decorators,
            is_abstract,
            name,
            type_parameters,
            extends,
            implements,
            members,
            span: start_span.combine(&end_span),
        }))
    }

    // Helper methods (pub so other parser modules can use them)

    pub(super) fn parse_identifier(&mut self) -> Result<Ident, ParserError> {
        match &self.current().kind {
            TokenKind::Identifier(name) => {
                let span = self.current_span();
                let ident = Spanned::new(name.clone(), span);
                self.advance();
                Ok(ident)
            }
            _ => Err(ParserError {
                message: format!("Expected identifier, got {:?}", self.current().kind),
                span: self.current_span(),
            }),
        }
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
            Statement::For(f) => match f {
                ForStatement::Numeric(n) => n.span,
                ForStatement::Generic(g) => g.span,
            },
            Statement::Repeat(r) => r.span,
            Statement::Return(r) => r.span,
            Statement::Break(s) | Statement::Continue(s) => *s,
            Statement::Expression(e) => e.span,
            Statement::Block(b) => b.span,
        }
    }
}
