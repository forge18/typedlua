local Error = {}
Error.__index = Error

function Error._init(self, message)
    self.message = message
    self.stack = debug.traceback()
end

function Error.new(message)
    local self = setmetatable({}, Error)
    Error._init(self, message)
    return self
end

function Error:toString()
    return self.message
end

-- Reflection metadata for Error
Error.__typeId = 1
Error.__typeName = "Error"
Error.__ancestors = {
    [1] = true,
}
Error.__ownFields = {
    { name = "message", flags = 0 },
    { name = "stack", flags = 0 },
}
Error.__ownMethods = {
    { name = "toString", flags = 0, arity = 0 },
}

function Error._buildAllFields()
    if Error.__allFieldsCache then
        return Error.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(Error.__ownFields) do
        table.insert(fields, field)
    end
    Error.__allFieldsCache = fields
    return fields
end

function Error._buildAllMethods()
    if Error.__allMethodsCache then
        return Error.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(Error.__ownMethods) do
        table.insert(methods, method)
    end
    Error.__allMethodsCache = methods
    return methods
end

local ArgumentError = {}
ArgumentError.__index = ArgumentError

setmetatable(ArgumentError, { __index = Error })

function ArgumentError._init(self, message, argumentName)
    Error._init(self, message)
    self.argumentName = argumentName
end

function ArgumentError.new(message, argumentName)
    local self = setmetatable({}, ArgumentError)
    ArgumentError._init(self, message, argumentName)
    return self
end

-- Reflection metadata for ArgumentError
ArgumentError.__typeId = 2
ArgumentError.__typeName = "ArgumentError"
ArgumentError.__parent = Error
ArgumentError.__ancestors = {
    [2] = true,
    -- Inherit ancestors from Error
    -- Note: Parent ancestors will be merged at class load time
}
if Error and Error.__ancestors then
    for ancestorId, _ in pairs(Error.__ancestors) do
        ArgumentError.__ancestors[ancestorId] = true
    end
end
ArgumentError.__ownFields = {
    { name = "argumentName", flags = 0 },
}
ArgumentError.__ownMethods = {
}

function ArgumentError._buildAllFields()
    if ArgumentError.__allFieldsCache then
        return ArgumentError.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(ArgumentError.__ownFields) do
        table.insert(fields, field)
    end
    if ArgumentError.__parent and ArgumentError.__parent._buildAllFields then
        local parentFields = ArgumentError.__parent._buildAllFields()
        for _, field in ipairs(parentFields) do
            table.insert(fields, field)
        end
    end
    ArgumentError.__allFieldsCache = fields
    return fields
end

function ArgumentError._buildAllMethods()
    if ArgumentError.__allMethodsCache then
        return ArgumentError.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(ArgumentError.__ownMethods) do
        table.insert(methods, method)
    end
    if ArgumentError.__parent and ArgumentError.__parent._buildAllMethods then
        local parentMethods = ArgumentError.__parent._buildAllMethods()
        for _, method in ipairs(parentMethods) do
            table.insert(methods, method)
        end
    end
    ArgumentError.__allMethodsCache = methods
    return methods
end

local StateError = {}
StateError.__index = StateError

setmetatable(StateError, { __index = Error })

function StateError._init(self, message)
    Error._init(self, message)
end

function StateError.new(message)
    local self = setmetatable({}, StateError)
    StateError._init(self, message)
    return self
end

-- Reflection metadata for StateError
StateError.__typeId = 3
StateError.__typeName = "StateError"
StateError.__parent = Error
StateError.__ancestors = {
    [3] = true,
    -- Inherit ancestors from Error
    -- Note: Parent ancestors will be merged at class load time
}
if Error and Error.__ancestors then
    for ancestorId, _ in pairs(Error.__ancestors) do
        StateError.__ancestors[ancestorId] = true
    end
end
StateError.__ownFields = {
}
StateError.__ownMethods = {
}

function StateError._buildAllFields()
    if StateError.__allFieldsCache then
        return StateError.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(StateError.__ownFields) do
        table.insert(fields, field)
    end
    if StateError.__parent and StateError.__parent._buildAllFields then
        local parentFields = StateError.__parent._buildAllFields()
        for _, field in ipairs(parentFields) do
            table.insert(fields, field)
        end
    end
    StateError.__allFieldsCache = fields
    return fields
end

function StateError._buildAllMethods()
    if StateError.__allMethodsCache then
        return StateError.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(StateError.__ownMethods) do
        table.insert(methods, method)
    end
    if StateError.__parent and StateError.__parent._buildAllMethods then
        local parentMethods = StateError.__parent._buildAllMethods()
        for _, method in ipairs(parentMethods) do
            table.insert(methods, method)
        end
    end
    StateError.__allMethodsCache = methods
    return methods
end

local IOError = {}
IOError.__index = IOError

setmetatable(IOError, { __index = Error })

function IOError._init(self, message, path)
    Error._init(self, message)
    self.path = path
end

function IOError.new(message, path)
    local self = setmetatable({}, IOError)
    IOError._init(self, message, path)
    return self
end

-- Reflection metadata for IOError
IOError.__typeId = 4
IOError.__typeName = "IOError"
IOError.__parent = Error
IOError.__ancestors = {
    [4] = true,
    -- Inherit ancestors from Error
    -- Note: Parent ancestors will be merged at class load time
}
if Error and Error.__ancestors then
    for ancestorId, _ in pairs(Error.__ancestors) do
        IOError.__ancestors[ancestorId] = true
    end
end
IOError.__ownFields = {
    { name = "path", flags = 0 },
}
IOError.__ownMethods = {
}

function IOError._buildAllFields()
    if IOError.__allFieldsCache then
        return IOError.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(IOError.__ownFields) do
        table.insert(fields, field)
    end
    if IOError.__parent and IOError.__parent._buildAllFields then
        local parentFields = IOError.__parent._buildAllFields()
        for _, field in ipairs(parentFields) do
            table.insert(fields, field)
        end
    end
    IOError.__allFieldsCache = fields
    return fields
end

function IOError._buildAllMethods()
    if IOError.__allMethodsCache then
        return IOError.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(IOError.__ownMethods) do
        table.insert(methods, method)
    end
    if IOError.__parent and IOError.__parent._buildAllMethods then
        local parentMethods = IOError.__parent._buildAllMethods()
        for _, method in ipairs(parentMethods) do
            table.insert(methods, method)
        end
    end
    IOError.__allMethodsCache = methods
    return methods
end

local ParseError = {}
ParseError.__index = ParseError

setmetatable(ParseError, { __index = Error })

function ParseError._init(self, message, line, column)
    Error._init(self, message)
    self.line = line
    self.column = column
end

function ParseError.new(message, line, column)
    local self = setmetatable({}, ParseError)
    ParseError._init(self, message, line, column)
    return self
end

-- Reflection metadata for ParseError
ParseError.__typeId = 5
ParseError.__typeName = "ParseError"
ParseError.__parent = Error
ParseError.__ancestors = {
    [5] = true,
    -- Inherit ancestors from Error
    -- Note: Parent ancestors will be merged at class load time
}
if Error and Error.__ancestors then
    for ancestorId, _ in pairs(Error.__ancestors) do
        ParseError.__ancestors[ancestorId] = true
    end
end
ParseError.__ownFields = {
    { name = "line", flags = 0 },
    { name = "column", flags = 0 },
}
ParseError.__ownMethods = {
}

function ParseError._buildAllFields()
    if ParseError.__allFieldsCache then
        return ParseError.__allFieldsCache
    end
    local fields = {}
    for _, field in ipairs(ParseError.__ownFields) do
        table.insert(fields, field)
    end
    if ParseError.__parent and ParseError.__parent._buildAllFields then
        local parentFields = ParseError.__parent._buildAllFields()
        for _, field in ipairs(parentFields) do
            table.insert(fields, field)
        end
    end
    ParseError.__allFieldsCache = fields
    return fields
end

function ParseError._buildAllMethods()
    if ParseError.__allMethodsCache then
        return ParseError.__allMethodsCache
    end
    local methods = {}
    for _, method in ipairs(ParseError.__ownMethods) do
        table.insert(methods, method)
    end
    if ParseError.__parent and ParseError.__parent._buildAllMethods then
        local parentMethods = ParseError.__parent._buildAllMethods()
        for _, method in ipairs(parentMethods) do
            table.insert(methods, method)
        end
    end
    ParseError.__allMethodsCache = methods
    return methods
end

local function require(condition, message)
    if (not condition) then
        error(ArgumentError.new(message))
    end
end
local function check(value, message)
    if ((value == nil)) then
        error(ArgumentError.new((message ~= nil and message or "Value cannot be nil")))
    end
    return value
end
local function unreachable(message)
    error(StateError.new((message ~= nil and message or "Reached unreachable code")))
end

local M = {}
M.Error = Error
M.ArgumentError = ArgumentError
M.StateError = StateError
M.IOError = IOError
M.ParseError = ParseError
M.require = require
M.check = check
M.unreachable = unreachable
return M
