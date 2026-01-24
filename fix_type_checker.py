#!/usr/bin/env python3
"""
Fix StringId/String type mismatches in type_checker.rs
Handles:
1. Add lifetime and interner/common fields to TypeChecker struct
2. Missing interner parameters in Lexer::new and Parser::new
3. StringId to String conversions using interner.resolve()
"""

import re
import sys

def add_interner_to_struct(content):
    """Add lifetime parameter and interner/common fields to TypeChecker struct"""

    # Add lifetime parameter to struct definition
    content = re.sub(
        r'pub struct TypeChecker \{',
        "pub struct TypeChecker<'a> {",
        content
    )

    # Add interner and common fields after diagnostic_handler
    content = re.sub(
        r'(    diagnostic_handler: Arc<dyn DiagnosticHandler>,\n)',
        r"\1    interner: &'a crate::string_interner::StringInterner,\n    common: &'a crate::string_interner::CommonIdentifiers,\n",
        content
    )

    return content

def update_constructor(content):
    """Update TypeChecker::new to accept and store interner and common"""

    # Update constructor signature
    old_sig = r'pub fn new\(diagnostic_handler: Arc<dyn DiagnosticHandler>\) -> Self \{'
    new_sig = """pub fn new(
        diagnostic_handler: Arc<dyn DiagnosticHandler>,
        interner: &'a crate::string_interner::StringInterner,
        common: &'a crate::string_interner::CommonIdentifiers,
    ) -> Self {"""

    content = re.sub(old_sig, new_sig, content)

    # Add interner and common to struct initialization
    content = re.sub(
        r'(            diagnostic_handler,\n)(        \};)',
        r'\1            interner,\n            common,\n\2',
        content
    )

    return content

def fix_lexer_parser_calls(content):
    """Add missing interner parameters to Lexer::new and Parser::new calls"""

    # Fix Lexer::new - add &self.interner as 3rd parameter
    # Match handler.clone() as a complete unit
    content = re.sub(
        r'Lexer::new\(([^,]+),\s*handler\.clone\(\)\)',
        r'Lexer::new(\1, handler.clone(), &self.interner)',
        content
    )

    # Fix Parser::new - add &self.interner and &self.common as 3rd and 4th parameters
    content = re.sub(
        r'Parser::new\(([^,]+),\s*handler\.clone\(\)\)',
        r'Parser::new(\1, handler.clone(), &self.interner, &self.common)',
        content
    )

    return content

def fix_stringid_conversions(content):
    """Convert StringId .node.clone() to interner.resolve().to_string()"""

    # General pattern: any .node.clone() that needs String conversion
    # This catches Symbol::new, function arguments expecting String, etc.
    # Match patterns like: something.name.node.clone(), ident.node.clone(), etc.

    # Pattern 1: Multi-level node access with clone
    content = re.sub(
        r'(\w+)\.(\w+)\.node\.clone\(\)',
        r'self.interner.resolve(\1.\2.node).to_string()',
        content
    )

    # Pattern 2: Single-level node access with clone
    content = re.sub(
        r'(\w+)\.node\.clone\(\)',
        r'self.interner.resolve(\1.node).to_string()',
        content
    )

    # Pattern 3: self.symbol_table.lookup(&something.node) -> need to resolve to &str
    content = re.sub(
        r'self\.symbol_table\.lookup\(&([a-z_]+\.(?:name|local|key)\.node)\)',
        r'self.symbol_table.lookup(self.interner.resolve(\1))',
        content
    )

    # Pattern 4: exports.add_named with bare identifier (already a StringId variable)
    # This handles cases where the variable itself is StringId, not .node
    # We need to be more careful here to not double-convert

    return content

def fix_impl_block(content):
    """Add lifetime parameter to impl TypeChecker block"""

    content = re.sub(
        r'impl TypeChecker \{',
        r"impl<'a> TypeChecker<'a> {",
        content
    )

    return content

def main():
    input_file = "crates/typedlua-core/src/typechecker/type_checker.rs"

    # Read the file
    with open(input_file, 'r') as f:
        content = f.read()

    # Apply fixes in order
    print("Step 1: Adding lifetime and interner/common fields to TypeChecker struct...")
    content = add_interner_to_struct(content)

    print("Step 2: Updating TypeChecker::new constructor...")
    content = update_constructor(content)

    print("Step 3: Adding lifetime to impl block...")
    content = fix_impl_block(content)

    print("Step 4: Adding interner parameters to Lexer::new and Parser::new...")
    content = fix_lexer_parser_calls(content)

    print("Step 5: Converting StringId to String using interner.resolve()...")
    content = fix_stringid_conversions(content)

    # Write back
    with open(input_file, 'w') as f:
        f.write(content)

    print(f"\n✓ Fixed {input_file}")
    print("\nRun 'cargo check -p typedlua-core' to verify fixes")

if __name__ == "__main__":
    main()
