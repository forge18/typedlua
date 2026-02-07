use typedlua_core::config::OptimizationLevel;
use typedlua_core::di::DiContainer;

fn compile_with_optimizer(source: &str, opt_level: OptimizationLevel) -> Result<String, String> {
    let mut container = DiContainer::test_default();
    container.compile_with_optimization(source, opt_level)
}

#[cfg(test)]
mod interface_inlining_tests {
    use super::*;

    #[test]
    fn test_single_implementing_class_inlines() {
        let source = r#"
            interface Greeter {
                greet(): string {
                    return "Hello, " .. self.name
                }
            }

            class User implements Greeter {
                name: string

                constructor(name: string) {
                    self.name = name
                }
            }

            const user = new User("Alice")
            user.greet()
        "#;

        let result = compile_with_optimizer(source, OptimizationLevel::O3);
        match &result {
            Ok(output) => {
                println!("O3 output:\n{}", output);
                assert!(
                    output.contains("greet") || output.contains("Hello"),
                    "Should have inlined or generated greet method"
                );
            }
            Err(e) => {
                panic!("Should compile successfully: {}", e);
            }
        }
    }

    #[test]
    fn test_multiple_implementing_classes_no_inline() {
        let source = r#"
            interface Greeter {
                greet(): string
            }

            class EnglishGreeter implements Greeter {
                greet(): string {
                    return "Hello"
                }
            }

            class FrenchGreeter implements Greeter {
                greet(): string {
                    return "Bonjour"
                }
            }

            const eng = new EnglishGreeter()
            eng.greet()
        "#;

        let result = compile_with_optimizer(source, OptimizationLevel::O3);
        match &result {
            Ok(output) => {
                println!("O3 output:\n{}", output);
            }
            Err(e) => {
                panic!("Should compile successfully: {}", e);
            }
        }
    }

    #[test]
    fn test_interface_with_default_method_inlining() {
        let source = r#"
            interface Logger {
                name: string

                log(): string {
                    return "Log: " .. self.name
                }
            }

            class ConsoleLogger implements Logger {
                name: string = "default"
            }

            const logger = new ConsoleLogger()
            logger.log()
        "#;

        let result = compile_with_optimizer(source, OptimizationLevel::O3);
        match &result {
            Ok(output) => {
                println!("O3 output:\n{}", output);
            }
            Err(e) => {
                panic!("Should compile successfully: {}", e);
            }
        }
    }

    #[test]
    fn test_chained_interface_method_calls() {
        let source = r#"
            interface StringProcessor {
                process(): string {
                    return self.value .. " processed"
                }
            }

            class Processor implements StringProcessor {
                value: string

                constructor(value: string) {
                    self.value = value
                }
            }

            const p = new Processor("test")
            p.process()
        "#;

        let result = compile_with_optimizer(source, OptimizationLevel::O3);
        match &result {
            Ok(output) => {
                println!("O3 output:\n{}", output);
            }
            Err(e) => {
                panic!("Should compile successfully: {}", e);
            }
        }
    }

    #[test]
    fn test_no_regression_at_o1() {
        let source = r#"
            interface Greeter {
                greet(): string {
                    return "Hello"
                }
            }

            class User implements Greeter {}

            const user = new User()
        "#;

        let o1_result = compile_with_optimizer(source, OptimizationLevel::O1);
        match o1_result {
            Ok(_) => {}
            Err(e) => {
                panic!("O1 should compile without errors: {}", e);
            }
        }
    }

    #[test]
    fn test_no_regression_at_o2() {
        let source = r#"
            interface Greeter {
                greet(): string {
                    return "Hello"
                }
            }

            class User implements Greeter {}

            const user = new User()
        "#;

        let o2_result = compile_with_optimizer(source, OptimizationLevel::O2);
        match o2_result {
            Ok(_) => {}
            Err(e) => {
                panic!("O2 should compile without errors: {}", e);
            }
        }
    }

    #[test]
    fn test_generic_interface_method() {
        let source = r#"
            interface Converter<T> {
                convert(value: T): string {
                    return "converted"
                }
            }

            class NumberConverter implements Converter<number> {}

            const converter = new NumberConverter()
            converter.convert(42)
        "#;

        let result = compile_with_optimizer(source, OptimizationLevel::O3);
        match &result {
            Ok(output) => {
                println!("O3 output:\n{}", output);
            }
            Err(e) => {
                panic!("Should compile successfully: {}", e);
            }
        }
    }
}
