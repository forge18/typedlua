//! Module system runtime for bundled output.

pub const MODULE_PRELUDE: &str = r#"-- Module registry and cache
local __modules = {}
local __cache = {}

-- Custom require function for bundled modules
local function __require(name)
    if __cache[name] then
        return __cache[name]
    end

    if not __modules[name] then
        error("Module not found: " .. name)
    end

    local exports = __modules[name]()
    __cache[name] = exports
    return exports
end
"#;
