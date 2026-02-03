use crate::diagnostics::CollectingDiagnosticHandler;
use std::rc::Rc;
use std::sync::Arc;
use typedlua_parser::string_interner::StringInterner;
use typedlua_parser::{Lexer, Parser};
use typedlua_typechecker::TypeChecker;

pub trait TypeCheckHelper {
    fn type_check_source(&self, source: &str) -> Result<(), String>;
}

impl TypeCheckHelper for super::di::Container {
    fn type_check_source(&self, source: &str) -> Result<(), String> {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let (mut interner, common_ids) = StringInterner::new_with_common_identifiers();
        let interner = Rc::new(interner);

        let mut lexer = Lexer::new(source, handler.clone(), &interner);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lexing failed: {:?}", e))?;

        let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
        let mut program = parser
            .parse()
            .map_err(|e| format!("Parsing failed: {:?}", e))?;

        let mut type_checker = TypeChecker::new(handler, &interner, &common_ids);
        type_checker
            .check_program(&mut program)
            .map_err(|e| e.message)?;

        Ok(())
    }
}
