//! Reflection runtime support for TypedLua.

pub const TYPE_REGISTRY: &str = r#"__TypeRegistry = {
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
    -- Try global lookup first (for registered types)
    _G = _G or getfenv(0)
    if _G[name] and _G[name].__typeName == name then
        return _G[name]
    end
    -- Try type registry lookup
    local typeId = __TypeRegistry[name]
    if typeId then
        -- Lookup by typeId would require reverse registry
        -- For now, rely on global lookup above
    end
    return nil
end
"#;
