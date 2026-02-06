use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;
use std::path::Path;
use typedlua_parser::ast::statement::{ExportKind, Statement};
use typedlua_parser::ast::Program;
use typedlua_parser::string_interner::{StringId, StringInterner};

#[derive(Debug, Clone)]
pub struct ReachableSet {
    pub modules: HashSet<String>,
    pub exports: HashMap<String, HashSet<String>>,
}

impl ReachableSet {
    pub fn new() -> Self {
        ReachableSet {
            modules: HashSet::default(),
            exports: HashMap::default(),
        }
    }

    pub fn is_module_reachable(&self, module_path: &str) -> bool {
        self.modules.contains(module_path)
    }

    pub fn is_export_reachable(&self, module_path: &str, export_name: &str) -> bool {
        self.exports
            .get(module_path)
            .map(|exports| exports.contains(export_name))
            .unwrap_or(false)
    }

    pub fn get_reachable_modules(&self) -> &HashSet<String> {
        &self.modules
    }

    pub fn get_reachable_exports(&self, module_path: &str) -> Option<&HashSet<String>> {
        self.exports.get(module_path)
    }
}

#[derive(Debug, Clone)]
pub struct ReachabilityAnalysis<'a> {
    interner: &'a StringInterner,
    export_map: HashMap<String, HashSet<(StringId, String)>>,
    import_map: HashMap<String, Vec<(String, StringId, String)>>,
    reachable_set: ReachableSet,
}

impl<'a> ReachabilityAnalysis<'a> {
    pub fn new(interner: &'a StringInterner) -> Self {
        ReachabilityAnalysis {
            interner,
            export_map: HashMap::default(),
            import_map: HashMap::default(),
            reachable_set: ReachableSet::new(),
        }
    }

    pub fn analyze(
        entry: &Path,
        modules: &HashMap<String, Program>,
        interner: &StringInterner,
    ) -> ReachableSet {
        let mut analysis = ReachabilityAnalysis::new(interner);

        if modules.is_empty() {
            return analysis.reachable_set;
        }

        let entry_str = Self::path_to_string(entry);
        analysis.build_export_maps(modules);
        analysis.build_import_maps(modules);

        let all_module_names: HashSet<String> = modules.keys().cloned().collect();

        let mut worklist: Vec<String> = vec![entry_str.clone()];
        analysis.reachable_set.modules.insert(entry_str.clone());

        while let Some(current_module) = worklist.pop() {
            let entry_exports = analysis
                .export_map
                .get(&current_module)
                .cloned()
                .unwrap_or_default();
            let exported_names: HashSet<String> =
                entry_exports.iter().map(|(_, name)| name.clone()).collect();
            analysis
                .reachable_set
                .exports
                .insert(current_module.clone(), exported_names);

            if let Some(imports) = analysis.import_map.get(&current_module) {
                for (source, _id, _name) in imports {
                    let resolved = Self::resolve_module_path(source, &all_module_names);
                    if let Some(resolved_path) = resolved {
                        if !analysis.reachable_set.modules.contains(&resolved_path) {
                            analysis.reachable_set.modules.insert(resolved_path.clone());
                            worklist.push(resolved_path);
                        }
                    }
                }
            }
        }

        analysis.compute_reachable_exports(entry_str);

        analysis.reachable_set
    }

    fn build_export_maps(&mut self, modules: &HashMap<String, Program>) {
        for (module_path, program) in modules {
            let mut exports = HashSet::default();
            self.collect_exports(program, &mut exports);
            if !exports.is_empty() {
                self.export_map.insert(module_path.clone(), exports);
            }
        }
    }

    fn collect_exports(&self, program: &Program, exports: &mut HashSet<(StringId, String)>) {
        for statement in &program.statements {
            match statement {
                Statement::Export(decl) => match &decl.kind {
                    ExportKind::Declaration(inner) => {
                        if let Some((id, name)) = self.get_declaration_name(inner) {
                            exports.insert((id, name));
                        }
                    }
                    ExportKind::Named { specifiers, .. } => {
                        for spec in specifiers {
                            let id = match &spec.exported {
                                Some(exported) => exported.node,
                                None => spec.local.node,
                            };
                            let name = self.resolve_string(id);
                            exports.insert((id, name));
                        }
                    }
                    ExportKind::Default(_expr) => {
                        let default_id = self.interner.get_or_intern("default");
                        exports.insert((default_id, "default".to_string()));
                    }
                },
                Statement::Function(decl) => {
                    let id = decl.name.node;
                    let name = self.resolve_string(id);
                    exports.insert((id, name));
                }
                Statement::Class(decl) => {
                    let id = decl.name.node;
                    let name = self.resolve_string(id);
                    exports.insert((id, name));
                }
                Statement::Enum(decl) => {
                    let id = decl.name.node;
                    let name = self.resolve_string(id);
                    exports.insert((id, name));
                }
                Statement::Interface(decl) => {
                    let id = decl.name.node;
                    let name = self.resolve_string(id);
                    exports.insert((id, name));
                }
                Statement::TypeAlias(decl) => {
                    let id = decl.name.node;
                    let name = self.resolve_string(id);
                    exports.insert((id, name));
                }
                Statement::Variable(decl) => {
                    if let typedlua_parser::ast::pattern::Pattern::Identifier(ident) = &decl.pattern
                    {
                        let id = ident.node;
                        let name = self.resolve_string(id);
                        exports.insert((id, name));
                    }
                }
                _ => {}
            }
        }
    }

    fn get_declaration_name(&self, stmt: &Statement) -> Option<(StringId, String)> {
        match stmt {
            Statement::Function(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Class(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Enum(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Interface(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::TypeAlias(decl) => {
                let id = decl.name.node;
                let name = self.resolve_string(id);
                Some((id, name))
            }
            Statement::Variable(decl) => {
                if let typedlua_parser::ast::pattern::Pattern::Identifier(ident) = &decl.pattern {
                    let id = ident.node;
                    let name = self.resolve_string(id);
                    Some((id, name))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn build_import_maps(&mut self, modules: &HashMap<String, Program>) {
        for (module_path, program) in modules {
            let mut imports = Vec::new();
            self.collect_imports(program, &mut imports);
            if !imports.is_empty() {
                self.import_map.insert(module_path.clone(), imports);
            }
        }
    }

    fn collect_imports(&self, program: &Program, imports: &mut Vec<(String, StringId, String)>) {
        for statement in &program.statements {
            if let Statement::Import(import_decl) = statement {
                let source = import_decl.source.clone();
                match &import_decl.clause {
                    typedlua_parser::ast::statement::ImportClause::Default(ident) => {
                        let id = ident.node;
                        let name = self.resolve_string(id);
                        imports.push((source, id, name));
                    }
                    typedlua_parser::ast::statement::ImportClause::Named(specifiers)
                    | typedlua_parser::ast::statement::ImportClause::TypeOnly(specifiers) => {
                        for spec in specifiers {
                            let id = spec.imported.node;
                            let name = self.resolve_string(id);
                            imports.push((source.clone(), id, name));
                        }
                    }
                    typedlua_parser::ast::statement::ImportClause::Namespace(ident) => {
                        let id = ident.node;
                        let name = self.resolve_string(id);
                        imports.push((source, id, name));
                    }
                    typedlua_parser::ast::statement::ImportClause::Mixed { default, named } => {
                        let default_id = default.node;
                        let default_name = self.resolve_string(default_id);
                        imports.push((source.clone(), default_id, default_name));
                        for spec in named {
                            let id = spec.imported.node;
                            let name = self.resolve_string(id);
                            imports.push((source.clone(), id, name));
                        }
                    }
                }
            }
        }
    }

    fn compute_reachable_exports(&mut self, entry_module: String) {
        let entry_exports = self
            .export_map
            .get(&entry_module)
            .cloned()
            .unwrap_or_default();
        let mut entry_all_exports: HashSet<String> =
            entry_exports.iter().map(|(_, name)| name.clone()).collect();

        let mut used_exports: HashMap<String, HashSet<String>> = HashMap::default();
        used_exports.insert(entry_module.clone(), entry_all_exports);

        let mut worklist: Vec<(String, String)> = Vec::new();
        worklist.push((entry_module.clone(), "__entry".to_string()));

        while let Some((current_module, _)) = worklist.pop() {
            if let Some(imports) = self.import_map.get(&current_module) {
                for (source, _id, imported_name) in imports {
                    if let Some(resolved_path) =
                        Self::resolve_module_path(source, &self.reachable_set.modules)
                    {
                        if self.reachable_set.modules.contains(&resolved_path) {
                            let source_exports = self
                                .export_map
                                .get(&resolved_path)
                                .cloned()
                                .unwrap_or_default();

                            let matching_export = source_exports
                                .iter()
                                .find(|(_id, name)| name == imported_name)
                                .map(|(_, name)| name.clone());

                            if let Some(export_name) = matching_export {
                                let module_used =
                                    used_exports.entry(resolved_path.clone()).or_default();
                                if !module_used.contains(&export_name) {
                                    module_used.insert(export_name.clone());
                                    worklist.push((resolved_path, export_name));
                                }
                            }
                        }
                    }
                }
            }
        }

        for module_path in &self.reachable_set.modules {
            let mut final_exports: HashSet<String> = HashSet::default();
            if module_path == &entry_module {
                let entry_exports = self
                    .export_map
                    .get(module_path)
                    .cloned()
                    .unwrap_or_default();
                final_exports = entry_exports.iter().map(|(_, name)| name.clone()).collect();
            }
            if let Some(used) = used_exports.get(module_path) {
                final_exports.extend(used.clone());
            }
            self.reachable_set
                .exports
                .insert(module_path.clone(), final_exports);
        }
    }

    fn path_to_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    fn resolve_module_path(source: &str, available_modules: &HashSet<String>) -> Option<String> {
        if available_modules.contains(source) {
            return Some(source.to_string());
        }
        let with_lua = format!("{}.lua", source);
        if available_modules.contains(&with_lua) {
            return Some(with_lua);
        }
        let with_index = format!("{}/index.lua", source);
        if available_modules.contains(&with_index) {
            return Some(with_index);
        }
        None
    }

    fn resolve_string(&self, id: StringId) -> String {
        self.interner.resolve(id).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::CollectingDiagnosticHandler;
    use std::sync::Arc;
    use typedlua_parser::lexer::Lexer;
    use typedlua_parser::parser::Parser;

    fn create_program(
        source: &str,
        interner: &StringInterner,
        common: &typedlua_parser::string_interner::CommonIdentifiers,
    ) -> Program {
        let handler = Arc::new(CollectingDiagnosticHandler::new());
        let mut lexer = Lexer::new(source, handler.clone(), interner);
        let tokens = lexer.tokenize().expect("Lexing failed");
        let mut parser = Parser::new(tokens, handler, interner, common);
        parser.parse().expect("Parsing failed")
    }

    #[test]
    fn test_single_module_reachable() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            export function add(a, b) return a + b end
            const x = 42
        "#;
        let program = create_program(source, &interner, &common);
        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(reachable.is_export_reachable("main.lua", "add"));
        assert!(reachable.is_export_reachable("main.lua", "x"));
    }

    #[test]
    fn test_simple_import_chain() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            import { add } from "math"
            const result = add(1, 2)
        "#;

        let math_source = r#"
            export function add(a, b) return a + b end
            export function sub(a, b) return a - b end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let math_program = create_program(math_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("math.lua".to_string(), math_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(reachable.is_module_reachable("math.lua"));
        assert!(reachable.is_export_reachable("math.lua", "add"));
    }

    #[test]
    fn test_unused_module_not_reachable() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            export function main() return 42 end
        "#;

        let unused_source = r#"
            export function unused() return 1 end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let unused_program = create_program(unused_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("unused.lua".to_string(), unused_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(!reachable.is_module_reachable("unused.lua"));
    }

    #[test]
    fn test_circular_dependencies() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let a_source = r#"
            import { b_fn } from "b"
            export function a_fn() return b_fn() end
        "#;

        let b_source = r#"
            import { a_fn } from "a"
            export function b_fn() return a_fn() end
        "#;

        let a_program = create_program(a_source, &interner, &common);
        let b_program = create_program(b_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("a.lua".to_string(), a_program);
        modules.insert("b.lua".to_string(), b_program);

        let entry = Path::new("a.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("a.lua"));
        assert!(reachable.is_module_reachable("b.lua"));
        assert!(reachable.is_export_reachable("a.lua", "a_fn"));
        assert!(reachable.is_export_reachable("b.lua", "b_fn"));
    }

    #[test]
    fn test_transitive_dependencies() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            import { greet } from "greeter"
            const x = greet("world")
        "#;

        let greeter_source = r#"
            import { formatGreeting } from "utils"
            export function greet(name) return formatGreeting(name) end
        "#;

        let utils_source = r#"
            export function formatGreeting(name) return "Hello, " .. name end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let greeter_program = create_program(greeter_source, &interner, &common);
        let utils_program = create_program(utils_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("greeter.lua".to_string(), greeter_program);
        modules.insert("utils.lua".to_string(), utils_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(reachable.is_module_reachable("greeter.lua"));
        assert!(reachable.is_module_reachable("utils.lua"));
        assert!(reachable.is_export_reachable("greeter.lua", "greet"));
        assert!(reachable.is_export_reachable("utils.lua", "formatGreeting"));
    }

    #[test]
    fn test_namespace_import() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            import * as math from "math"
            const x = math.add(1, 2)
        "#;

        let math_source = r#"
            export function add(a, b) return a + b end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let math_program = create_program(math_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("math.lua".to_string(), math_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(reachable.is_module_reachable("math.lua"));
    }

    #[test]
    fn test_default_export() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            import default from "module"
            const x = default()
        "#;

        let module_source = r#"
            export default function() return 42 end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let module_program = create_program(module_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("module.lua".to_string(), module_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("main.lua"));
        assert!(reachable.is_module_reachable("module.lua"));
    }

    #[test]
    fn test_entry_point_always_reachable() {
        let (interner, common) = StringInterner::new_with_common_identifiers();
        let source = r#"
            function internal() return 1 end
        "#;
        let program = create_program(source, &interner, &common);
        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("deeply/nested/module.lua".to_string(), program);

        let entry = Path::new("deeply/nested/module.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        assert!(reachable.is_module_reachable("deeply/nested/module.lua"));
    }

    #[test]
    fn test_reachable_exports_output() {
        let (interner, common) = StringInterner::new_with_common_identifiers();

        let main_source = r#"
            import { used } from "dep"
            export const result = used()
        "#;

        let dep_source = r#"
            export function used() return 1 end
            export function unused() return 2 end
        "#;

        let main_program = create_program(main_source, &interner, &common);
        let dep_program = create_program(dep_source, &interner, &common);

        let mut modules: HashMap<String, Program> = HashMap::default();
        modules.insert("main.lua".to_string(), main_program);
        modules.insert("dep.lua".to_string(), dep_program);

        let entry = Path::new("main.lua");
        let reachable = ReachabilityAnalysis::analyze(entry, &modules, &interner);

        let main_exports = reachable.get_reachable_exports("main.lua").unwrap();
        assert!(main_exports.contains("result"));

        let dep_exports = reachable.get_reachable_exports("dep.lua").unwrap();
        assert!(dep_exports.contains("used"));
        assert!(!dep_exports.contains("unused"));
    }
}
