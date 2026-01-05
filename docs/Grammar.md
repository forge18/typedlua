# TypedLua Formal Grammar

**Document Version:** 0.1
**Last Updated:** 2024-12-31

This document defines the complete formal grammar for TypedLua using Extended Backus-Naur Form (EBNF) notation.

## Note on Comments

TypedLua uses **Lua-style comments**:
- Single-line: `--` (not C-style `//`)
- Multi-line: `--[[ ... ]]--` (not C-style `/* ... */`)

This design choice avoids conflicts with the `//` integer division operator introduced in Lua 5.3 and maintains consistency with the target language.

---

## EBNF Notation

```
| = alternation (or)
? = optional (zero or one)
* = zero or more
+ = one or more
() = grouping
[] = character class
"" = literal string
'' = literal character
```

---

## Program Structure

```ebnf
Program = Statement* EOF

Statement = VariableDeclaration
          | FunctionDeclaration
          | ClassDeclaration
          | InterfaceDeclaration
          | TypeAliasDeclaration
          | EnumDeclaration
          | ImportDeclaration
          | ExportDeclaration
          | ExpressionStatement
          | IfStatement
          | WhileStatement
          | ForStatement
          | ReturnStatement
          | BreakStatement
          | ContinueStatement
          | Block
```

---

## Declarations

### Variable Declaration

```ebnf
VariableDeclaration = ("const" | "local") Identifier (":" Type)? "=" Expression

Destructuring = ArrayDestructure | ObjectDestructure

ArrayDestructure = "[" DestructureElement ("," DestructureElement)* "]"
DestructureElement = Identifier | "..." Identifier | "_"

ObjectDestructure = "{" ObjectDestructureElement ("," ObjectDestructureElement)* "}"
ObjectDestructureElement = Identifier (":" Identifier)? ("=" Expression)?
```

### Function Declaration

```ebnf
FunctionDeclaration = "function" Identifier TypeParameters? 
                      "(" ParameterList? ")" (":" Type)? 
                      Block

ParameterList = Parameter ("," Parameter)*
Parameter = Identifier ":" Type ("=" Expression)?
          | "..." Identifier ":" Type

TypeParameters = "<" TypeParameter ("," TypeParameter)* ">"
TypeParameter = Identifier ("extends" Type)? ("=" Type)?
```

### Interface Declaration

```ebnf
InterfaceDeclaration = "interface" Identifier TypeParameters? 
                       ("extends" TypeList)? 
                       "{" InterfaceMember* "}"

InterfaceMember = PropertySignature | MethodSignature | IndexSignature

PropertySignature = ("readonly")? Identifier "?" ":" Type ","?

MethodSignature = Identifier TypeParameters? 
                  "(" ParameterList? ")" ":" Type ","?

IndexSignature = "[" Identifier ":" ("string" | "number") "]" ":" Type ","?
```

### Type Alias

```ebnf
TypeAliasDeclaration = "type" Identifier TypeParameters? "=" Type
```

### Enum Declaration

```ebnf
EnumDeclaration = "enum" Identifier "{" EnumMember ("," EnumMember)* ","? "}"

EnumMember = Identifier ("=" (NumberLiteral | StringLiteral))?
```

### Class Declaration (OOP)

```ebnf
ClassDeclaration = Decorator* ("abstract")? ("@sealed")? 
                   "class" Identifier TypeParameters? 
                   ("extends" Type)? 
                   ("implements" TypeList)? 
                   "{" ClassMember* "}"

ClassMember = PropertyDeclaration
            | ConstructorDeclaration
            | MethodDeclaration
            | GetterDeclaration
            | SetterDeclaration

PropertyDeclaration = Decorator* AccessModifier? ("static")? ("readonly")? 
                      Identifier ":" Type ("=" Expression)? ","?

ConstructorDeclaration = Decorator* "constructor" "(" ParameterList? ")" Block

MethodDeclaration = Decorator* AccessModifier? ("static")? ("abstract")? 
                    Identifier TypeParameters? 
                    "(" ParameterList? ")" ":" Type 
                    Block?

GetterDeclaration = Decorator* AccessModifier? ("static")? 
                    "get" Identifier "(" ")" ":" Type Block

SetterDeclaration = Decorator* AccessModifier? ("static")? 
                    "set" Identifier "(" Parameter ")" Block

AccessModifier = "public" | "private" | "protected"
```

### Decorator (Decorators)

```ebnf
Decorator = "@" (Identifier | CallExpression)
```

### Import/Export Declarations

```ebnf
ImportDeclaration = "import" ImportClause "from" StringLiteral

ImportClause = Identifier                              // default import
             | "{" ImportSpecifier ("," ImportSpecifier)* "}"  // named imports
             | "*" "as" Identifier                     // namespace import
             | "type" "{" ImportSpecifier ("," ImportSpecifier)* "}"  // type-only

ImportSpecifier = Identifier ("as" Identifier)?

ExportDeclaration = "export" (Declaration | ExportClause | "default" Expression)

ExportClause = "{" ExportSpecifier ("," ExportSpecifier)* "}"
ExportSpecifier = Identifier ("as" Identifier)?
```

---

## Types

```ebnf
Type = PrimaryType ("|" PrimaryType)*     // Union type

PrimaryType = IntersectionType ("&" IntersectionType)*

IntersectionType = PostfixType

PostfixType = PrimitiveType
            | ObjectType
            | ArrayType
            | TupleType
            | FunctionType
            | TypeReference
            | LiteralType
            | TypeQuery
            | ConditionalType
            | MappedType
            | TemplateLiteralType
            | "(" Type ")"
            | PostfixType "?"        // Nullable (T | nil)
            | PostfixType "[" "]"    // Array type shorthand
            | PostfixType "[" Type "]"  // Index access type

PrimitiveType = "nil" | "boolean" | "number" | "integer" | "string" 
              | "unknown" | "never" | "void" | "table" | "coroutine"

ObjectType = "{" ObjectMember* "}"

ObjectMember = PropertySignature | MethodSignature | IndexSignature

ArrayType = "Array" "<" Type ">"
          | Type "[" "]"

TupleType = "[" Type ("," Type)* "]"

FunctionType = "(" ParameterList? ")" "->" Type

TypeReference = Identifier TypeArguments?

TypeArguments = "<" Type ("," Type)* ">"

LiteralType = StringLiteral | NumberLiteral | BooleanLiteral | "nil"

TypeQuery = "typeof" Expression

ConditionalType = Type "extends" Type "?" Type ":" Type

MappedType = "{" ("readonly")? "[" Identifier "in" Type "]" ("?")? ":" Type "}"

TemplateLiteralType = "`" TemplateLiteralPart* "`"
TemplateLiteralPart = TemplateChars | "${" Type "}"

KeyofType = "keyof" Type

TypeList = Type ("," Type)*
```

---

## Expressions

```ebnf
Expression = AssignmentExpression

AssignmentExpression = ConditionalExpression (AssignmentOperator ConditionalExpression)?

AssignmentOperator = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "..="

ConditionalExpression = PipeExpression ("?" Expression ":" Expression)?

PipeExpression = LogicalOrExpression ("|>" LogicalOrExpression)*

LogicalOrExpression = LogicalAndExpression ("or" LogicalAndExpression)*

LogicalAndExpression = EqualityExpression ("and" EqualityExpression)*

EqualityExpression = RelationalExpression (("==" | "~=") RelationalExpression)*

RelationalExpression = AdditiveExpression (("<" | ">" | "<=" | ">=") AdditiveExpression)*

AdditiveExpression = MultiplicativeExpression (("+" | "-") MultiplicativeExpression)*

MultiplicativeExpression = UnaryExpression (("*" | "/" | "%" | "//") UnaryExpression)*

UnaryExpression = ("not" | "-" | "#") UnaryExpression
                | PostfixExpression

PostfixExpression = PrimaryExpression (PostfixOperator)*

PostfixOperator = "." Identifier               // Member access
                | "[" Expression "]"           // Index access
                | "(" ArgumentList? ")"        // Function call
                | ":" Identifier "(" ArgumentList? ")"  // Method call
                | "{" ObjectLiteral "}"        // Table constructor

ArgumentList = Argument ("," Argument)*
Argument = Expression | "..." Expression

PrimaryExpression = Identifier
                  | Literal
                  | FunctionExpression
                  | ClassExpression
                  | MatchExpression
                  | ArrayLiteral
                  | ObjectLiteral
                  | ParenthesizedExpression
                  | "self"
                  | "super"

Literal = NumberLiteral
        | StringLiteral
        | BooleanLiteral
        | "nil"
        | TemplateLiteral

NumberLiteral = DecimalLiteral | HexLiteral | BinaryLiteral

StringLiteral = '"' StringCharacter* '"'
              | "'" StringCharacter* "'"

TemplateLiteral = "`" TemplatePart* "`"
TemplatePart = TemplateChars | "${" Expression "}"

BooleanLiteral = "true" | "false"

ArrayLiteral = "{" (Expression | "..." Expression) ("," (Expression | "..." Expression))* ","? "}"

ObjectLiteral = "{" ObjectProperty* "}"
ObjectProperty = Identifier "=" Expression ","?
               | "[" Expression "]" "=" Expression ","?
               | "..." Expression ","?

FunctionExpression = "function" TypeParameters? "(" ParameterList? ")" (":" Type)? Block
                   | "(" ParameterList? ")" "=>" (Expression | Block)

ClassExpression = "class" Identifier? TypeParameters? 
                  ("extends" Type)? 
                  ("implements" TypeList)? 
                  "{" ClassMember* "}"

MatchExpression = "match" Expression "{" MatchArm ("," MatchArm)* ","? "}"

MatchArm = Pattern ("when" Expression)? "=>" (Expression | Block)

Pattern = LiteralPattern
        | IdentifierPattern
        | ObjectPattern
        | ArrayPattern
        | WildcardPattern

LiteralPattern = Literal

IdentifierPattern = Identifier

ObjectPattern = "{" ObjectPatternProperty ("," ObjectPatternProperty)* "}"
ObjectPatternProperty = Identifier (":" Pattern)?

ArrayPattern = "[" Pattern ("," Pattern)* ("," "..." Identifier)? "]"

WildcardPattern = "_"

ParenthesizedExpression = "(" Expression ")"
```

---

## Statements

```ebnf
IfStatement = "if" Expression "then" Block 
              ("elseif" Expression "then" Block)* 
              ("else" Block)? 
              "end"

WhileStatement = "while" Expression "do" Block "end"

ForStatement = ForNumericStatement | ForGenericStatement

ForNumericStatement = "for" Identifier "=" Expression "," Expression ("," Expression)? 
                      "do" Block "end"

ForGenericStatement = "for" IdentifierList "in" ExpressionList "do" Block "end"

ReturnStatement = "return" ExpressionList?

BreakStatement = "break"

ContinueStatement = "continue"

Block = "{" Statement* "}"
      | Statement*  // For Lua-style blocks without braces

ExpressionStatement = Expression

IdentifierList = Identifier ("," Identifier)*

ExpressionList = Expression ("," Expression)*
```

---

## Lexical Elements

```ebnf
Identifier = IdentifierStart IdentifierPart*

IdentifierStart = Letter | "_"

IdentifierPart = Letter | Digit | "_"

Letter = [a-zA-Z]

Digit = [0-9]

DecimalLiteral = Digit+ ("." Digit+)? (("e" | "E") ("+" | "-")? Digit+)?

HexLiteral = "0x" HexDigit+

HexDigit = [0-9a-fA-F]

BinaryLiteral = "0b" ("0" | "1")+

StringCharacter = EscapeSequence | [^"\\\n]

EscapeSequence = "\\" ("n" | "t" | "r" | "\\" | '"' | "'" | "0" | "x" HexDigit HexDigit)

TemplateChars = TemplateChar+

TemplateChar = [^`$\\] | "\\" . | "$" [^{]

Comment = LineComment | BlockComment

LineComment = "--" [^\n]* "\n"

BlockComment = "--[[" (. | \n)* "]]" "--"?

Whitespace = [ \t\n\r]+
```

---

## Keywords

```ebnf
Keyword = "abstract" | "and" | "as" | "assert" | "break" | "class" 
        | "const" | "continue" | "declare" | "default" | "do" 
        | "else" | "elseif" | "end" | "enum" | "export" | "extends"
        | "false" | "for" | "from" | "function" | "get" | "if" 
        | "implements" | "import" | "in" | "interface" | "local"
        | "match" | "module" | "nil" | "not" | "or" | "private"
        | "protected" | "public" | "readonly" | "return" | "self"
        | "set" | "static" | "super" | "then" | "true" | "type"
        | "typeof" | "void" | "when" | "while"

// Primitive type keywords
PrimitiveKeyword = "boolean" | "number" | "integer" | "string" 
                 | "table" | "coroutine" | "unknown" | "never"

// Reserved for future use
Reserved = "async" | "await" | "yield" | "namespace" | "package"
```

---

## Operator Precedence (Highest to Lowest)

1. Postfix: `.`, `[]`, `()`, `:`
2. Unary: `not`, `-`, `#`
3. Multiplicative: `*`, `/`, `%`, `//`
4. Additive: `+`, `-`
5. Relational: `<`, `>`, `<=`, `>=`
6. Equality: `==`, `~=`
7. Logical AND: `and`
8. Logical OR: `or`
9. Pipe: `|>`
10. Conditional: `?:`
11. Assignment: `=`, `+=`, `-=`, etc.

---

## Grammar Extensions by Feature Flag

### OOP Features (`enableOOP: true`)

When OOP is enabled, these productions are valid:
- `ClassDeclaration`
- `AccessModifier`
- `ConstructorDeclaration`
- `GetterDeclaration`
- `SetterDeclaration`
- Keywords: `class`, `extends`, `implements`, `abstract`, `public`, `private`, `protected`, `static`, `super`, `get`, `set`

### FP Features (`enableFP: true`)

When FP is enabled, these productions are valid:
- `MatchExpression`
- `PipeExpression` (the `|>` operator)
- `ArrayDestructure`
- `ObjectDestructure`
- Spread operator in `ArrayLiteral` and `ObjectLiteral`
- Rest parameters in `ParameterList`
- Keywords: `match`, `when`

### Decorator Features (`enableDecorators: true`)

When decorators are enabled, these productions are valid:
- `Decorator`
- Decorators can appear before: `ClassDeclaration`, `PropertyDeclaration`, `MethodDeclaration`, etc.

---

**Document Version:** 0.1  
**Last Updated:** 2024-12-31
