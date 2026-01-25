//! Rich enum runtime support for TypedLua.
//! Uses `{}` placeholder for enum name - replace before use.

pub const ENUM_NAME: &str = r#"function {}:name()
    return self.__name
end
"#;

pub const ENUM_ORDINAL: &str = r#"function {}:ordinal()
    return self.__ordinal
end
"#;

pub const ENUM_VALUES: &str = r#"function {}.values()
    return {}.__values
end
"#;

pub const ENUM_VALUE_OF: &str = r#"function {}.valueOf(name)
    return {}.__byName[name]
end
"#;
