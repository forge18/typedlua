//! Decorator runtime support for TypedLua.

pub const DECORATOR_RUNTIME: &str = r#"-- TypedLua Runtime Library
-- Provides built-in decorators and runtime helpers

local TypedLua = {}

-- ============================================================================
-- @readonly Decorator
-- ============================================================================

-- Makes a class property readonly by intercepting property writes
function TypedLua.readonly(target)
    if type(target) ~= "table" then
        return target
    end

    local mt = getmetatable(target) or {}
    local original_newindex = mt.__newindex

    -- Store which properties exist at decoration time
    local readonly_props = {}
    for k, _ in pairs(target) do
        readonly_props[k] = true
    end

    -- Intercept property writes
    mt.__newindex = function(t, key, value)
        if readonly_props[key] then
            error("Cannot modify readonly property '" .. tostring(key) .. "'", 2)
        end
        -- Allow new properties to be added (for non-readonly ones)
        if original_newindex then
            original_newindex(t, key, value)
        else
            rawset(t, key, value)
        end
    end

    return setmetatable(target, mt)
end

-- ============================================================================
-- @sealed Decorator
-- ============================================================================

-- Prevents adding new properties or methods to a class
function TypedLua.sealed(target)
    if type(target) ~= "table" then
        return target
    end

    local mt = getmetatable(target) or {}
    local original_newindex = mt.__newindex

    -- Store which properties exist at decoration time
    local allowed_props = {}
    for k, _ in pairs(target) do
        allowed_props[k] = true
    end

    -- Intercept property writes
    mt.__newindex = function(t, key, value)
        if not allowed_props[key] then
            error("Cannot add property '" .. tostring(key) .. "' to sealed class", 2)
        end
        -- Allow modifying existing properties
        if original_newindex then
            original_newindex(t, key, value)
        else
            rawset(t, key, value)
        end
    end

    return setmetatable(target, mt)
end

-- ============================================================================
-- @deprecated Decorator
-- ============================================================================

-- Decorator factory that takes an optional message
function TypedLua.deprecated(message)
    -- If called with a string, return a decorator function
    if type(message) == "string" then
        return function(target)
            return TypedLua._markDeprecated(target, message)
        end
    else
        -- If called directly on target, use default message
        return TypedLua._markDeprecated(message, nil)
    end
end

-- Internal function to mark something as deprecated
function TypedLua._markDeprecated(target, customMessage)
    if type(target) == "function" then
        -- Wrap functions to emit warning on call
        return function(...)
            local msg = customMessage or "This function is deprecated"
            io.stderr:write("Warning: " .. msg .. "\n")
            return target(...)
        end
    elseif type(target) == "table" then
        -- For classes/tables, wrap methods
        local wrapped = {}
        local mt = getmetatable(target) or {}

        -- Store original target reference
        local original_target = target

        -- Copy all properties
        for k, v in pairs(target) do
            if type(v) == "function" and k ~= "new" then
                -- Wrap methods (except constructor)
                wrapped[k] = function(...)
                    local msg = customMessage or ("Method '" .. tostring(k) .. "' is deprecated")
                    io.stderr:write("Warning: " .. msg .. "\n")
                    return v(...)
                end
            else
                wrapped[k] = v
            end
        end

        -- If there's a constructor, wrap it too
        if target.new then
            wrapped.new = function(...)
                local msg = customMessage or "This class is deprecated"
                io.stderr:write("Warning: " .. msg .. "\n")
                return target.new(...)
            end
        end

        -- Preserve metatable
        return setmetatable(wrapped, mt)
    end

    return target
end

-- Export to global scope if not already defined (allows user overrides)
if not readonly then
    readonly = TypedLua.readonly
end

if not sealed then
    sealed = TypedLua.sealed
end

if not deprecated then
    deprecated = TypedLua.deprecated
end

return TypedLua
"#;
