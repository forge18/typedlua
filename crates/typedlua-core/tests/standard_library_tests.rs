use std::rc::Rc;
use std::sync::Arc;
use typedlua_core::codegen::CodeGenerator;
use typedlua_core::config::{CompilerOptions, LuaVersion};
use typedlua_core::diagnostics::CollectingDiagnosticHandler;
use typedlua_core::typechecker::TypeChecker;
use typedlua_parser::lexer::Lexer;
use typedlua_parser::parser::Parser;
use typedlua_parser::string_interner::StringInterner;

fn compile_and_check(source: &str) -> Result<String, String> {
    // Use default target (Lua 5.4) to avoid stdlib reloading issues
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let mut type_checker = TypeChecker::new(handler.clone(), &interner, &common_ids);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

fn compile_with_target(source: &str, target: LuaVersion) -> Result<String, String> {
    let handler = Arc::new(CollectingDiagnosticHandler::new());
    let (interner, common_ids) = StringInterner::new_with_common_identifiers();
    let interner = Rc::new(interner);

    let mut lexer = Lexer::new(source, handler.clone(), &interner);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Lexing failed: {:?}", e))?;

    let mut parser = Parser::new(tokens, handler.clone(), &interner, &common_ids);
    let mut program = parser
        .parse()
        .map_err(|e| format!("Parsing failed: {:?}", e))?;

    let options = CompilerOptions {
        target,
        ..CompilerOptions::default()
    };

    let mut type_checker =
        TypeChecker::new(handler.clone(), &interner, &common_ids).with_options(options);
    type_checker
        .check_program(&mut program)
        .map_err(|e| e.message)?;

    let mut codegen = CodeGenerator::new(interner.clone());
    let output = codegen.generate(&mut program);

    Ok(output)
}

// ============================================================================
// Function Overloads - string.find
// ============================================================================

#[test]
fn test_string_find_two_args() {
    let source = r#"
        const text = "hello world"
        const startPos = string.find(text, "world")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.find with 2 args should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_find_three_args() {
    let source = r#"
        const text = "hello world"
        const startPos = string.find(text, "world", 7)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.find with 3 args should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_find_four_args() {
    let source = r#"
        const text = "hello world"
        const startPos, endPos = string.find(text, "world", 1, true)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.find with 4 args should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Function Overloads - string.sub
// ============================================================================

#[test]
fn test_string_sub_two_args() {
    let source = r#"
        const text = "hello world"
        const sub = string.sub(text, 7)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.sub with 2 args should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_sub_three_args() {
    let source = r#"
        const text = "hello world"
        const sub = string.sub(text, 1, 5)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.sub with 3 args should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Function Overloads - table.insert
// ============================================================================

#[test]
fn test_table_insert_two_args() {
    let source = r#"
        const arr = [1, 2, 3]
        table.insert(arr, 4)
    "#;

    let result = compile_and_check(source);
    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }
    assert!(
        result.is_ok(),
        "table.insert with 2 args should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_table_insert_three_args() {
    let source = r#"
        const arr = [1, 2, 3]
        table.insert(arr, 2, 99)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "table.insert with 3 args should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Function Overloads - tonumber
// ============================================================================

#[test]
fn test_tonumber_one_arg() {
    let source = r#"
        const num = tonumber("42")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "tonumber with 1 arg should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_tonumber_two_args() {
    let source = r#"
        const num = tonumber("1010", 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "tonumber with 2 args should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Variadic Functions - print
// ============================================================================

#[test]
fn test_print_no_args() {
    let source = r#"
        print()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "print with no args should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_print_single_arg() {
    let source = r#"
        print("hello")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "print with 1 arg should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_print_multiple_args() {
    let source = r#"
        print("hello", "world", 42, true)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "print with multiple args should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Variadic Functions - string.format
// ============================================================================

#[test]
fn test_string_format_simple() {
    let source = r#"
        const msg = string.format("Hello, %s!", "world")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.format should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_format_multiple_placeholders() {
    let source = r#"
        const msg = string.format("Name: %s, Age: %d, Score: %.2f", "Alice", 25, 95.5)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.format with multiple placeholders should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_format_integer() {
    let source = r#"
        const hex = string.format("0x%X", 255)
        const binary = string.format("0b%b", 5)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.format with integer formats should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Variadic Functions - select
// ============================================================================

#[test]
fn test_select_count() {
    let source = r#"
        function countArgs(...): number {
            return select("count", ...)
        }
        
        const count = countArgs(1, 2, 3, 4, 5)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "select with count should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_select_index() {
    let source = r#"
        function getThird(...) {
            return select(3, ...)
        }
        
        const third = getThird("a", "b", "c", "d", "e")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "select with index should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Lua 5.1 Specific APIs
// ============================================================================

#[test]
fn test_lua51_getfenv_setfenv() {
    let source = r#"
        function testFunction(): void {
            print("test")
        }
        
        const env = getfenv(testFunction)
        setfenv(testFunction, {})
    "#;

    let result = compile_with_target(source, LuaVersion::Lua51);
    assert!(
        result.is_ok(),
        "getfenv/setfenv for Lua 5.1 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua51_unpack() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const a, b, c = unpack(arr)
    "#;

    let result = compile_with_target(source, LuaVersion::Lua51);
    assert!(
        result.is_ok(),
        "unpack for Lua 5.1 should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Lua 5.2 Specific APIs
// ============================================================================

#[test]
fn test_lua52_table_pack() {
    let source = r#"
        const packed = table.pack(1, 2, 3, 4, 5)
        const count = packed.n
    "#;

    let result = compile_with_target(source, LuaVersion::Lua52);
    assert!(
        result.is_ok(),
        "table.pack for Lua 5.2 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua52_table_unpack() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const a, b, c = table.unpack(arr)
    "#;

    let result = compile_with_target(source, LuaVersion::Lua52);
    assert!(
        result.is_ok(),
        "table.unpack for Lua 5.2 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua52_bit32() {
    let source = r#"
        const a = 0xFF
        const b = 0x0F
        const band = bit32.band(a, b)
        const bor = bit32.bor(a, b)
        const bxor = bit32.bxor(a, b)
        const bnot = bit32.bnot(a)
        const lshift = bit32.lshift(a, 2)
        const rshift = bit32.rshift(a, 2)
    "#;

    let result = compile_with_target(source, LuaVersion::Lua52);
    assert!(
        result.is_ok(),
        "bit32 library for Lua 5.2 should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Lua 5.3 Specific APIs
// ============================================================================

#[test]
fn test_lua53_math_tointeger() {
    let source = r#"
        const int = math.tointeger(3.14)
        const fromStr = math.tointeger("42")
    "#;

    let result = compile_with_target(source, LuaVersion::Lua53);
    assert!(
        result.is_ok(),
        "math.tointeger for Lua 5.3 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua53_math_type() {
    let source = r#"
        const intType = math.type(42)
        const floatType = math.type(3.14)
    "#;

    let result = compile_with_target(source, LuaVersion::Lua53);
    assert!(
        result.is_ok(),
        "math.type for Lua 5.3 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua53_integer_ops() {
    let source = r#"
        const a = 10
        const b = 3
        const div = a // b
        const mod = a % b
    "#;

    let result = compile_with_target(source, LuaVersion::Lua53);
    assert!(
        result.is_ok(),
        "Integer operators for Lua 5.3 should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_lua53_utf8() {
    let source = r#"
        const utf8len = utf8.len("hello")
        const codepoint = utf8.codepoint("hello", 1)
        const offset = utf8.offset("hello", 2)
        const char = utf8.char(65, 66, 67)
    "#;

    let result = compile_with_target(source, LuaVersion::Lua53);
    assert!(
        result.is_ok(),
        "utf8 library for Lua 5.3 should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Lua 5.4 Specific APIs
// ============================================================================

#[test]
fn test_lua54_warn() {
    let source = r#"
        warn("This is a warning message")
        warn("Multiple", "warning", "messages")
    "#;

    let result = compile_with_target(source, LuaVersion::Lua54);
    assert!(
        result.is_ok(),
        "warn() for Lua 5.4 should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Basic Functions
// ============================================================================

#[test]
fn test_basic_type() {
    let source = r#"
        const t1 = type("hello")
        const t2 = type(42)
        const t3 = type(true)
        const t4 = type({})
        const t5 = type(nil)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "type() function should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_basic_tostring() {
    let source = r#"
        const str1 = tostring(42)
        const str2 = tostring(true)
        const str3 = tostring(nil)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "tostring() function should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_basic_pairs_ipairs() {
    let source = r#"
        const obj = { a: 1, b: 2, c: 3 }
        const arr = [10, 20, 30]
        
        for (const k, v of pairs(obj)) {
            print(k, v)
        }
        
        for (const i, v of ipairs(arr)) {
            print(i, v)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "pairs() and ipairs() should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_basic_next() {
    let source = r#"
        const obj = { a: 1, b: 2 }
        const firstKey, firstVal = next(obj)
        const secondKey, secondVal = next(obj, firstKey)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "next() function should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_basic_rawget_rawset() {
    let source = r#"
        const t = {}
        rawset(t, "key", "value")
        const val = rawget(t, "key")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "rawget/rawset should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_basic_rawequal() {
    let source = r#"
        const a = {}
        const b = a
        const c = {}
        const isEqual = rawequal(a, b)
        const notEqual = rawequal(a, c)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "rawequal() should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Math Functions
// ============================================================================

#[test]
fn test_math_basic_functions() {
    let source = r#"
        const abs = math.abs(-5)
        const floor = math.floor(3.7)
        const ceil = math.ceil(3.2)
        const sqrt = math.sqrt(16)
        const pow = math.pow(2, 3)
        const max = math.max(1, 5, 3, 9, 2)
        const min = math.min(1, 5, 3, 9, 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic math functions should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_math_trigonometric() {
    let source = r#"
        const pi = math.pi
        const sin = math.sin(pi / 2)
        const cos = math.cos(0)
        const tan = math.tan(pi / 4)
        const asin = math.asin(1)
        const acos = math.acos(1)
        const atan = math.atan(1)
        const atan2 = math.atan(1, 1)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Trigonometric math functions should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_math_random() {
    let source = r#"
        math.randomseed(os.time())
        const r1 = math.random()
        const r2 = math.random(100)
        const r3 = math.random(1, 6)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "math.random functions should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Table Functions
// ============================================================================

#[test]
fn test_table_concat() {
    let source = r#"
        const arr = ["a", "b", "c", "d"]
        const joined = table.concat(arr)
        const joinedWithSep = table.concat(arr, ", ")
        const joinedSlice = table.concat(arr, "-", 2, 3)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "table.concat should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_table_remove() {
    let source = r#"
        const arr = [1, 2, 3, 4, 5]
        const last = table.remove(arr)
        const second = table.remove(arr, 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "table.remove should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_table_sort() {
    let source = r#"
        const arr = [3, 1, 4, 1, 5, 9, 2, 6]
        table.sort(arr)
        table.sort(arr, (a, b) => b - a)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "table.sort should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - String Functions
// ============================================================================

#[test]
fn test_string_basic_functions() {
    let source = r#"
        const strLen = string.len("hello")
        const upper = string.upper("hello")
        const lower = string.lower("WORLD")
        const rep = string.rep("*", 5)
        const reverse = string.reverse("hello")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic string functions should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_byte_char() {
    let source = r#"
        const byte1, byte2 = string.byte("hello", 1, 2)
        const char1 = string.char(65, 66, 67)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.byte and string.char should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_gsub() {
    let source = r#"
        const text = "hello world"
        const result, count = string.gsub(text, "l", "L")
        const withLimit = string.gsub(text, "l", "L", 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.gsub should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_match() {
    let source = r#"
        const text = "hello 123 world"
        const m1 = string.match(text, "%d+")
        const m2, m3 = string.match(text, "(%d+).-(%a+)")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.match should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_string_gmatch() {
    let source = r#"
        const text = "hello world test"
        for (const word of string.gmatch(text, "%a+")) {
            print(word)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "string.gmatch should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - OS Functions
// ============================================================================

#[test]
fn test_os_time() {
    let source = r#"
        const now = os.time()
        const specific = os.time({ year: 2024, month: 1, day: 15 })
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "os.time should compile: {:?}", result.err());
}

#[test]
fn test_os_date() {
    let source = r#"
        const now = os.date()
        const formatted = os.date("%Y-%m-%d")
        const dateTable = os.date("*t")
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "os.date should compile: {:?}", result.err());
}

#[test]
fn test_os_clock() {
    let source = r#"
        const cpu = os.clock()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "os.clock should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - IO Functions
// ============================================================================

#[test]
fn test_io_basic() {
    let source = r#"
        io.write("Hello, ")
        io.write("World!\n")
        io.flush()
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic io functions should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_io_file_operations() {
    let source = r#"
        const file = io.open("test.txt", "w")
        if (file) {
            file:write("hello")
            file:close()
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "io file operations should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Coroutine Functions
// ============================================================================

#[test]
fn test_coroutine_basic() {
    let source = r#"
        const co = coroutine.create(function(): number {
            return 42
        })
        const success, result = coroutine.resume(co)
        const status = coroutine.status(co)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Basic coroutine functions should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_coroutine_yield() {
    let source = r#"
        const co = coroutine.create(function(): void {
            for (const i of [1, 2, 3]) {
                coroutine.yield(i)
            }
        })
        
        while (coroutine.status(co) != "dead") {
            const _, val = coroutine.resume(co)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "coroutine.yield should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_coroutine_wrap() {
    let source = r#"
        const wrapped = coroutine.wrap(function(n: number): number
            return n * 2
        end)
        
        const result = wrapped(21)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "coroutine.wrap should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Debug Functions
// ============================================================================

#[test]
fn test_debug_getinfo() {
    let source = r#"
        function test(): void {
            const info = debug.getinfo(1)
            print(info.name)
        }
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "debug.getinfo should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_debug_traceback() {
    let source = r#"
        const trace = debug.traceback()
        const withMessage = debug.traceback("Error occurred")
        const withLevel = debug.traceback("Error", 2)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "debug.traceback should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Package Functions
// ============================================================================

#[test]
fn test_require() {
    let source = r#"
        const http = require("socket.http")
        const json = require("json")
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "require() should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_package_path() {
    let source = r#"
        const path = package.path
        const cpath = package.cpath
        package.path = package.path .. ";./?.lua"
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "package.path/cpath should compile: {:?}",
        result.err()
    );
}

// ============================================================================
// Standard Library - Metatable Functions
// ============================================================================

#[test]
fn test_setmetatable_getmetatable() {
    let source = r#"
        const t = {}
        const mt = {
            __index = function(t: any, k: any): any {
                return "not found"
            }
        }
        
        setmetatable(t, mt)
        const retrieved = getmetatable(t)
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "setmetatable/getmetatable should compile: {:?}",
        result.err()
    );
}

#[test]
fn test_rawset_rawget_metamethods() {
    let source = r#"
        const t = {}
        setmetatable(t, {
            __newindex = function(t: any, k: any, v: any): void {
                rawset(t, k, v)
            }
        })
    "#;

    let result = compile_and_check(source);
    assert!(
        result.is_ok(),
        "Metamethods with rawset should compile: {:?}",
        result.err()
    );
}
