//! Reflection runtime support for TypedLua.

pub const TYPE_REGISTRY: &str = r#"__TypeRegistry = {}
__TypeIdToClass = {}
"#;

pub const REFLECTION_MODULE: &str = r#"-- ============================================================
-- Reflection Runtime Module
-- ============================================================
Reflect = {}

-- O(1) instanceof check using ancestors table
function Reflect.isInstance(obj, typeName)
    if type(obj) ~= "table" or not obj.__ancestors then
        return false
    end
    local typeId = __TypeRegistry[typeName]
    if not typeId then return false end
    return obj.__ancestors[typeId] == true
end

function Reflect.typeof(obj)
    if type(obj) == "table" and obj.__typeName then
        return {
            id = obj.__typeId,
            name = obj.__typeName,
            kind = "class"
        }
    end
    return nil
end

function Reflect.getFields(obj)
    if type(obj) == "table" and obj._buildAllFields then
        return obj:_buildAllFields()
    end
    return {}
end

function Reflect.getMethods(obj)
    if type(obj) == "table" and obj._buildAllMethods then
        return obj:_buildAllMethods()
    end
    return {}
end

function Reflect.forName(name)
    -- Try type registry lookup first (O(1) via reverse registry)
    local typeId = __TypeRegistry[name]
    if typeId then
        local classConstructor = __TypeIdToClass[typeId]
        if classConstructor then
            return classConstructor
        end
    end
    -- Fallback to global lookup for dynamically created types
    _G = _G or getfenv(0)
    if _G[name] and _G[name].__typeName == name then
        return _G[name]
    end
    return nil
end

-- Decode bit flags from _flags field into a table of booleans
function Reflect.decodeFlags(flags)
    return {
        isPublic = (flags % 2) >= 1,
        isPrivate = (math.floor(flags / 2) % 2) >= 1,
        isProtected = (math.floor(flags / 4) % 2) >= 1,
        isReadonly = (math.floor(flags / 8) % 2) >= 1,
        isStatic = (math.floor(flags / 16) % 2) >= 1,
    }
end

-- Type code lookup for parseParams
local _typeCodes = {
    n = "number",
    s = "string",
    b = "boolean",
    t = "table",
    f = "function",
    v = "void",
    o = "any",
}

-- Parse compact parameter signature string into type names
function Reflect.parseParams(paramStr)
    local result = {}
    local i = 1
    while i <= #paramStr do
        local ch = paramStr:sub(i, i)
        local typeName = _typeCodes[ch]
        if typeName then
            result[#result + 1] = typeName
            i = i + 1
        else
            -- Unknown code, skip
            i = i + 1
        end
    end
    return result
end
"#;
