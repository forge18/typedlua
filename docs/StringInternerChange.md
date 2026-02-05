Plan: Migrate StringInterner to lasso::ThreadedRodeo for Parallel Parsing
Goal
Replace the custom StringInterner (which uses RefCell and is not thread-safe) with lasso::ThreadedRodeo to enable true parallel file parsing without needing StringId remapping or pre-scan approaches.

Why lasso?
Purpose-built concurrent string interner used by swc, oxc, and other Rust parsers
Thread-safe interning with minimal contention
Eliminates need for ~800 lines of AST remapping code
Eliminates coupling risk of pre-scan approach
Well-tested and maintained
Phase 1: Add lasso dependency
File: crates/typedlua-parser/Cargo.toml


[dependencies]
lasso = { version = "0.7", features = ["multi-threaded"] }
Phase 2: Create compatibility wrapper
File: crates/typedlua-parser/src/string_interner.rs

Create a new implementation that wraps ThreadedRodeo but maintains the existing API:


use lasso::{ThreadedRodeo, Spur};

/// StringId wraps lasso's Spur for backward compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(Spur);

pub struct StringInterner {
    rodeo: ThreadedRodeo,
}

impl StringInterner {
    pub fn new() -> Self {
        Self { rodeo: ThreadedRodeo::new() }
    }

    pub fn intern(&self, s: &str) -> StringId {
        StringId(self.rodeo.get_or_intern(s))
    }

    pub fn resolve(&self, id: StringId) -> String {
        self.rodeo.resolve(&id.0).to_string()
    }

    pub fn with_resolved<F, R>(&self, id: StringId, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        f(self.rodeo.resolve(&id.0))
    }
}
Key changes:

Remove RefCell wrappers (ThreadedRodeo is inherently thread-safe)
StringId now wraps lasso::Spur instead of u32
Remove merge_from method (no longer needed)
Keep all existing public methods with same signatures
Phase 3: Update CommonIdentifiers
File: crates/typedlua-parser/src/string_interner.rs

The CommonIdentifiers struct and new_with_common_identifiers() should work unchanged since they just call intern().

Verify these methods still compile:

new_with_common_identifiers() -> (StringInterner, CommonIdentifiers)
All keyword StringIds in CommonIdentifiers
Phase 4: Handle Serialization
Current: StringInterner has to_strings() and from_strings() for cache serialization.

With lasso: Need to serialize/deserialize the rodeo contents.


impl StringInterner {
    /// For cache serialization
    pub fn to_strings(&self) -> Vec<String> {
        self.rodeo.strings().map(|s| s.to_string()).collect()
    }

    /// For cache deserialization
    pub fn from_strings(strings: Vec<String>) -> Self {
        let rodeo = ThreadedRodeo::new();
        for s in strings {
            rodeo.get_or_intern(s);
        }
        Self { rodeo }
    }
}
Phase 5: Update parallel parsing code
File: crates/typedlua-cli/src/main.rs (or wherever multi-file compilation happens)

Now parsing can share a single interner across threads:


use rayon::prelude::*;

// Single shared interner (ThreadedRodeo is Send + Sync)
let interner = StringInterner::new_with_common_identifiers();

let parsed_modules: Vec<ParsedModule> = source_files
    .par_iter()
    .map(|file| {
        // All threads share the same interner - no merging needed!
        let tokens = Lexer::new(&file.content, &interner).tokenize()?;
        let ast = Parser::new(tokens, &interner, &common).parse()?;
        Ok(ParsedModule { path: file.path.clone(), ast })
    })
    .collect::<Result<Vec<_>, Error>>()?;
Key difference from current plan:

No per-thread interners
No merge_from calls
No remap tables
No AST StringId remapping visitor
Phase 6: Remove dead code
Delete these no-longer-needed items:

merge_from() method in StringInterner
Any StringId remapping visitor code (if partially implemented)
ParsedModule.interner field (if it stored per-module interner)
Files to Modify
File	Changes
crates/typedlua-parser/Cargo.toml	Add lasso dependency
crates/typedlua-parser/src/string_interner.rs	Replace implementation with ThreadedRodeo wrapper
crates/typedlua-parser/src/lib.rs	Verify re-exports still work
crates/typedlua-cli/src/main.rs	Update parallel parsing to share single interner
Verification
cargo build - Ensure everything compiles
cargo test -p typedlua-parser - Run interner tests
cargo test - Full test suite
Verify caching still works (to_strings/from_strings)
Test parallel parsing on a multi-file project
Notes
lasso::Spur is Copy, Eq, Hash - same as current StringId(u32)
ThreadedRodeo uses DashMap internally for concurrent access
Memory layout is slightly different but API is compatible
If you need Ord on StringId, lasso's Spur supports it
