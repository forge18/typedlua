use std::rc::Rc;
use std::collections::HashSet;
use crate::config::OptimizationLevel;
use crate::optimizer::OptimizationPass;
use typedlua_parser::ast::Program;
use typedlua_parser::ast::statement::{Block, Statement, VariableDeclaration};
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

use std::rc::Rc;
use std::collections::HashSet;
use crate::config::OptimizationLevel;
use crate::optimizer::OptimizationPass;
use typedlua_parser::ast::Program;
use typedlua_parser::ast::statement::{Block, Statement, VariableDeclaration};
use typedlua_parser::ast::Spanned;
use typedlua_parser::span::Span;
use typedlua_parser::string_interner::StringInterner;

