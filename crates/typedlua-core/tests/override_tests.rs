use typedlua_core::di::DiContainer;

fn compile(source: &str) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile(source)
}

fn type_check(source: &str) -> Result<(), String> {
    let mut container = DiContainer::test_default();
    container.compile_with_stdlib(source)?;
    Ok(())
}

#[test]
fn test_override_basic() {
    let source = r#"
        class Base {
            public method(): void {
                print("base")
            }
        }

        class Derived extends Base {
            override method(): void {
                print("derived")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Basic override should compile");
}

#[test]
fn test_override_return_type_covariance() {
    let source = r#"
        class Base {
            public getValue(): Base {
                return new Base()
            }
        }

        class Derived extends Base {
            override getValue(): Derived {
                return new Derived()
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Covariant return should compile");
}

#[test]
fn test_override_without_decorator_fails() {
    let source = r#"
        class Base {
            public method(): void {
            }
        }

        class Derived extends Base {
            public method(): void {
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_err(), "Override without decorator should fail");
}

#[test]
fn test_override_non_existent_method_fails() {
    let source = r#"
        class Base {
        }

        class Derived extends Base {
            override nonExistent(): void {
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_err(), "Override non-existent should fail");
}

#[test]
fn test_override_signature_mismatch_fails() {
    let source = r#"
        class Base {
            public method(a: number): void {
            }
        }

        class Derived extends Base {
            override method(a: string): void {
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_err(), "Signature mismatch should fail");
}

#[test]
fn test_override_final_method_fails() {
    let source = r#"
        class Base {
            public final method(): void {
            }
        }

        class Derived extends Base {
            override method(): void {
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_err(), "Override final should fail");
}

#[test]
fn test_override_abstract_method() {
    let source = r#"
        abstract class Base {
            public abstract method(): void
        }

        class Derived extends Base {
            override method(): void {
                print("implemented")
            }
        }
    "#;

    let result = type_check(source);
    assert!(result.is_ok(), "Override abstract should compile");
}

#[test]
fn test_multi_level_override() {
    let source = r#"
        class Base {
            public method(): void {
                print("base")
            }
        }

        class Middle extends Base {
            override method(): void {
                print("middle")
            }
        }

        class Derived extends Middle {
            override method(): void {
                print("derived")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Multi-level override should compile");
}

#[test]
fn test_override_with_super() {
    let source = r#"
        class Base {
            public value: number = 0

            constructor() {
                self.value = 1
            }

            public method(): void {
                print(self.value)
            }
        }

        class Derived extends Base {
            constructor() {
                super()
            }

            override method(): void {
                super.method()
                print("derived")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override with super should compile");
}

#[test]
fn test_override_getter() {
    let source = r#"
        class Base {
            protected _value: number = 0

            public get value(): number {
                return self._value
            }
        }

        class Derived extends Base {
            override get value(): number {
                return self._value * 2
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override getter should compile");
}

#[test]
fn test_override_setter() {
    let source = r#"
        class Base {
            protected _value: number = 0

            public set value(v: number) {
                self._value = v
            }
        }

        class Derived extends Base {
            override set value(v: number) {
                self._value = v * 2
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override setter should compile");
}

#[test]
fn test_override_property() {
    let source = r#"
        class Base {
            public value: number = 0
        }

        class Derived extends Base {
            override value: number = 10
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override property should compile");
}

#[test]
fn test_override_static_method() {
    let source = r#"
        class Base {
            public static method(): void {
                print("base")
            }
        }

        class Derived extends Base {
            override static method(): void {
                print("derived")
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override static method should compile");
}

#[test]
fn test_override_parameter_contravariance() {
    let source = r#"
        class Base {
            public method(a: Derived): void {
            }
        }

        class Derived extends Base {
            override method(a: Base): void {
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Contravariant parameters should compile");
}

#[test]
fn test_override_generic_method() {
    let source = r#"
        class Base {
            public method<T>(x: T): T {
                return x
            }
        }

        class Derived extends Base {
            override method<T>(x: T): T {
                return x
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override generic method should compile");
}

#[test]
fn test_override_with_different_arity() {
    let source = r#"
        class Base {
            public method(a: number, b: number): number {
                return a + b
            }
        }

        class Derived extends Base {
            override method(a: number, b: number): number {
                return a * b
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override with same arity should compile");
}

#[test]
fn test_override_private_method() {
    let source = r#"
        class Base {
            private helper(): void {
            }

            public method(): void {
                self.helper()
            }
        }

        class Derived extends Base {
            private helper(): void {
            }

            public method(): void {
                self.helper()
            }
        }
    "#;

    let result = compile(source);
    assert!(result.is_ok(), "Override private method should compile");
}
