use typedlua_core::di::DiContainer;

fn compile_and_check(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

#[test]
fn test_rich_enum_with_constructor_args() {
    let source = r#"
        enum Planet {
            Mercury(3.303e23, 2.4397e6),
            Venus(4.869e24, 6.0518e6),
            Earth(5.972e24, 6.371e6),
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with constructor args should compile");
}

#[test]
fn test_rich_enum_with_methods() {
    let source = r#"
        enum Status {
            Success,
            Error(string),
            Loading(progress: number),
        }

        impl Status {
            public isError(): boolean {
                match self {
                    Error(_) => true,
                    _ => false,
                }
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with methods should compile");
}

#[test]
fn test_rich_enum_with_static_methods() {
    let source = r#"
        enum Result<T, E> {
            Ok(T),
            Err(E),
        }

        impl Result<T, E> {
            public static ok(value: T): Result<T, E> {
                return Ok(value)
            }

            public static err(error: E): Result<T, E> {
                return Err(error)
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with static methods should compile");
}

#[test]
fn test_rich_enum_pattern_matching() {
    let source = r#"
        enum Option<T> {
            Some(T),
            None,
        }

        function unwrap<T>(opt: Option<T>): T
            match opt {
                Some(value) => value,
                None => error("unwrap None"),
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum pattern matching should compile");
}

#[test]
fn test_rich_enum_nested() {
    let source = r#"
        enum Tree<T> {
            Leaf(T),
            Node(Tree<T>, Tree<T>),
            Empty,
        }

        function height<T>(tree: Tree<T>): number
            match tree {
                Empty => 0,
                Leaf(_) => 1,
                Node(left, right) => 1 + max(height(left), height(right)),
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Nested enum should compile");
}

#[test]
fn test_rich_enum_with_interface() {
    let source = r#"
        interface Drawable {
            draw(): void
        }

        enum Shape {
            Circle(radius: number),
            Rectangle(width: number, height: number),
        }

        impl Shape: Drawable {
            public draw(): void {
                match self {
                    Circle(r) => const _ = r,
                    Rectangle(w, h) => const _ = w + h,
                }
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum implementing interface should compile");
}

#[test]
fn test_rich_enum_generic() {
    let source = r#"
        enum Either<L, R> {
            Left(L),
            Right(R),
        }

        function fromNullable<T>(value: T | nil): Either<T, string>
            if value ~= nil then
                return Left(value)
            end
            return Right("value is nil")
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Generic enum should compile");
}

#[test]
fn test_rich_enum_with_properties() {
    let source = r#"
        enum Config {
            Development,
            Production,
            Custom(host: string, port: number),
        }

        impl Config {
            public isLocal(): boolean {
                match self {
                    Development => true,
                    _ => false,
                }
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with properties should compile");
}

#[test]
fn test_rich_enum_exhaustive_match() {
    let source = r#"
        enum State {
            Idle,
            Running,
            Paused,
            Stopped,
        }

        function handle(state: State): string
            match state {
                Idle => "idle",
                Running => "running",
                Paused => "paused",
                Stopped => "stopped",
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Exhaustive enum match should compile");
}

#[test]
fn test_rich_enum_partial_match() {
    let source = r#"
        enum Response {
            Success,
            Failure(string),
        }

        function handle(response: Response): boolean
            match response {
                Failure(msg) => false,
                _ => true,
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Partial enum match should compile");
}

#[test]
fn test_rich_enum_with_default() {
    let source = r#"
        enum Color {
            Red,
            Green,
            Blue,
            Other(string),
        }

        function name(c: Color): string
            match c {
                Red => "red",
                Green => "green",
                Blue => "blue",
                Other(name) => name,
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with default should compile");
}

#[test]
fn test_rich_enum_recursive() {
    let source = r#"
        enum List<T> {
            Nil,
            Cons(T, List<T>),
        }

        function length<T>(list: List<T>): number
            match list {
                Nil => 0,
                Cons(_, rest) => 1 + length(rest),
            }
        end
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Recursive enum should compile");
}

#[test]
fn test_rich_enum_iterator() {
    let source = r#"
        enum Maybe<T> {
            Some(T),
            None,
        }

        impl<T> Maybe<T> {
            public iterator(): () => T | nil {
                return () => nil
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum iterator should compile");
}

#[test]
fn test_rich_enum_derives() {
    let source = r#"
        enum LogLevel {
            Debug,
            Info,
            Warning,
            Error,
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Simple enum should compile");
}

#[test]
fn test_rich_enum_with_self() {
    let source = r#"
        enum Expr {
            Number(number),
            Add(Expr, Expr),
            Mul(Expr, Expr),
        }

        impl Expr {
            public evaluate(): number {
                match self {
                    Number(n) => n,
                    Add(a, b) => a.evaluate() + b.evaluate(),
                    Mul(a, b) => a.evaluate() * b.evaluate(),
                }
            }
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Enum with self reference should compile");
}

#[test]
fn test_rich_enum_export() {
    let source = r#"
        export enum Status {
            Pending,
            Active,
            Done,
        }
    "#;

    let result = compile_and_check(source);
    assert!(result.is_ok(), "Exported enum should compile");
}
