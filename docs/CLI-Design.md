# TypedLua CLI Design

**Document Version:** 0.1  
**Last Updated:** 2024-12-31

This document defines the command-line interface for the TypedLua compiler, following TypeScript's `tsc` design for familiarity.

---

## Overview

The TypedLua compiler provides a single command `tl` (short for TypedLua) that mirrors TypeScript's `tsc` command structure.

**Binary name:** `tl`

**Design philosophy:** Maximum compatibility with TypeScript workflows - developers familiar with `tsc` should feel immediately at home.

---

## Basic Usage

### Compile Project

```bash
# Compile using typedlua.json in current directory
tl

# Same as above (explicit)
tl --project .

# Use specific config file
tl --project path/to/typedlua.json
tl -p ./config/typedlua.json

# Also accepts tsconfig.json for familiarity
tl -p tsconfig.json
```

### Compile Specific Files

```bash
# Compile single file
tl main.tl

# Compile multiple files
tl src/main.tl src/utils.tl lib/helper.tl

# Compile with glob patterns
tl src/**/*.tl
```

### Initialize Project

```bash
# Create typedlua.json with defaults
tl --init

# Creates:
# {
#   "compilerOptions": {
#     "target": "lua5.4",
#     "outDir": "./dist",
#     "sourceMap": true,
#     "strictNullChecks": true,
#     "enableOOP": true,
#     "enableFP": true,
#     "enableDecorators": true,
#     "allowNonTypedLua": true
#   },
#   "include": ["src/**/*"],
#   "exclude": ["node_modules", "dist"]
# }
```

---

[continuing in next message due to length...]

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
