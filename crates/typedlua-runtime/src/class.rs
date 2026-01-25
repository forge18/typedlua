//! Class runtime support for TypedLua.
//! Uses `{}` placeholder for class name - replace before use.

pub const BUILD_ALL_FIELDS: &str = r#"function {}._buildAllFields()
    if {}._allFieldsCache then return {}._allFieldsCache end
    local fields = {}
    local idx = 1

    -- Copy own fields
    for _, f in ipairs({}.__ownFields) do
        fields[idx] = f
        idx = idx + 1
    end

    -- Inherit fields from parent
    local parent = {}.__parent
    if parent and parent._buildAllFields then
        local inherited = parent:_buildAllFields()
        for _, f in ipairs(inherited) do
            fields[idx] = f
            idx = idx + 1
        end
    end

    {}._allFieldsCache = fields
    return fields
end
"#;

pub const BUILD_ALL_METHODS: &str = r#"function {}._buildAllMethods()
    if {}._allMethodsCache then return {}._allMethodsCache end
    local methods = {}
    local idx = 1

    -- Copy own methods
    for _, m in ipairs({}.__ownMethods) do
        methods[idx] = m
        idx = idx + 1
    end

    -- Inherit methods from parent
    local parent = {}.__parent
    if parent and parent._buildAllMethods then
        local inherited = parent:_buildAllMethods()
        for _, m in ipairs(inherited) do
            methods[idx] = m
            idx = idx + 1
        end
    end

    {}._allMethodsCache = methods
    return methods
end
"#;
