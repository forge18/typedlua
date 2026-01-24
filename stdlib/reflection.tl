// TypedLua Reflection Runtime API
// This module provides runtime reflection utilities for TypedLua classes

export class Runtime {
    /**
     * Check if an object is an instance of a given class type
     * O(1) operation using pre-computed ancestor table
     */
    static isInstance(obj: any, Type: any): boolean {
        const mt = getmetatable(obj)
        if (!mt || !mt.__typeId) {
            return false
        }

        if (!Type.__ancestors) {
            return false
        }

        return Type.__ancestors[mt.__typeId] == true
    }

    /**
     * Get type information for an object
     * Returns null for non-class instances (primitives, tables, etc.)
     * O(1) operation
     */
    static typeof(obj: any): TypeInfo | nil {
        const mt = getmetatable(obj)
        if (!mt || !mt.__typeName) {
            return nil
        }

        return {
            id: mt.__typeId,
            name: mt.__typeName,
            parent: mt.__parent,
            getFields: function(): FieldInfo[] {
                return mt._buildAllFields()
            },
            getMethods: function(): MethodInfo[] {
                return mt._buildAllMethods()
            },
            getOwnFields: function(): FieldInfo[] {
                return mt.__ownFields
            },
            getOwnMethods: function(): MethodInfo[] {
                return mt.__ownMethods
            }
        }
    }

    /**
     * Get all fields (including inherited) for a class Type
     * First call: O(n) where n = number of ancestors
     * Subsequent calls: O(1) from cache
     */
    static getFields(Type: any): FieldInfo[] {
        if (!Type._buildAllFields) {
            return []
        }
        return Type._buildAllFields()
    }

    /**
     * Get all methods (including inherited) for a class Type
     * First call: O(n) where n = number of ancestors
     * Subsequent calls: O(1) from cache
     */
    static getMethods(Type: any): MethodInfo[] {
        if (!Type._buildAllMethods) {
            return []
        }
        return Type._buildAllMethods()
    }

    /**
     * Get only the fields declared in the class itself (not inherited)
     * O(1) operation
     */
    static getOwnFields(Type: any): FieldInfo[] {
        return Type.__ownFields || []
    }

    /**
     * Get only the methods declared in the class itself (not inherited)
     * O(1) operation
     */
    static getOwnMethods(Type: any): MethodInfo[] {
        return Type.__ownMethods || []
    }
}

export interface TypeInfo {
    id: number
    name: string
    parent: any | nil
    getFields(): FieldInfo[]
    getMethods(): MethodInfo[]
    getOwnFields(): FieldInfo[]
    getOwnMethods(): MethodInfo[]
}

export interface FieldInfo {
    name: string
    // Future: type, optional, readonly flags
}

export interface MethodInfo {
    name: string
    isStatic: boolean
    // Future: parameters, returnType, isAbstract
}

// Helper function declarations for Lua built-ins
declare function getmetatable(obj: any): any | nil
