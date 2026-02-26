# Contributing to Riku

Thank you for your interest in contributing to Riku! This document outlines the process for contributing to the project.

## Getting Started

### Prerequisites

- Rust toolchain (latest stable)
- Cargo
- Git
- Basic knowledge of systems administration concepts

### Setting Up Development Environment

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/riku.git
   cd riku
   ```

3. Build the project:
   ```bash
   cargo build
   ```

4. Run tests to ensure everything works:
   ```bash
   cargo test
   ```

## Development Workflow

### Making Changes

1. Create a new branch for your feature or bug fix:
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b bugfix/issue-description
   ```

2. Make your changes, ensuring:
   - Code follows Rust idioms and conventions
   - All tests pass
   - New functionality is covered by tests
   - Code is properly documented

3. Run the test suite:
   ```bash
   cargo test
   ```

4. Check code formatting:
   ```bash
   cargo fmt
   ```

5. Check for common issues:
   ```bash
   cargo clippy
   ```

6. Commit your changes with a descriptive message:
   ```bash
   git add .
   git commit -m "Add feature: description of your feature"
   ```

7. Push to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```

8. Open a Pull Request on GitHub

## Code Style

### Rust Style

- Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/)
- Use `cargo fmt` for consistent formatting
- Use `cargo clippy` to catch common mistakes and improve code
- Write documentation for all public functions and structs
- Include examples in documentation where appropriate

### Naming Conventions

- Use `snake_case` for function and variable names
- Use `PascalCase` for type names and enum variants
- Use `SCREAMING_SNAKE_CASE` for constants

### Documentation

All public functions and structs should have documentation:

```rust
/// Brief description of the function.
///
/// More detailed explanation if needed.
///
/// # Arguments
///
/// * `param1` - Description of param1
/// * `param2` - Description of param2
///
/// # Returns
///
/// Description of return value
///
/// # Example
///
/// ```
/// let result = my_function(42, "hello");
/// assert_eq!(result, true);
/// ```
pub fn my_function(param1: i32, param2: &str) -> bool {
    // implementation
}
```

## Testing

### Writing Tests

- Write unit tests for all functions
- Use the `#[cfg(test)]` attribute for test modules
- Use descriptive test names
- Test both success and error cases
- Use `assert!`, `assert_eq!`, and `assert_ne!` macros for assertions

Example test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function_success() {
        let result = my_function(42, "hello");
        assert_eq!(result, true);
    }

    #[test]
    fn test_my_function_error() {
        let result = my_function(-1, "");
        assert_eq!(result, false);
    }
}
```

### Running Tests

- Run all tests: `cargo test`
- Run specific test: `cargo test test_name`
- Run tests with output: `cargo test -- --nocapture`
- Run tests in release mode: `cargo test --release`

## Architecture Guidelines

### Module Organization

- Keep related functionality in the same module
- Use clear, descriptive module names
- Maintain a flat hierarchy where possible
- Separate concerns (CLI, deployment, configuration, etc.)

### Error Handling

- Use `anyhow` for application-level errors
- Use `thiserror` for library-level errors
- Provide meaningful error messages
- Handle errors gracefully where possible

### Configuration

- Use the `RikuPaths` struct for all path resolution
- Follow the same directory structure as the original Piku
- Maintain backward compatibility with existing configurations

## Adding New Runtime Support

To add support for a new runtime:

1. Add a new variant to the `Runtime` enum in `deploy/mod.rs`
2. Create a new module in `deploy/` (e.g., `deploy/newlang.rs`)
3. Implement the deployment function in the new module
4. Update the `detect_runtime` function to recognize the new runtime
5. Update the `do_deploy` function to handle the new runtime
6. Add tests for the new runtime
7. Update documentation

## Adding New Commands

To add a new CLI command:

1. Add the command to the `Commands` enum in `cli/mod.rs`
2. Create a handler function in the appropriate module
3. Update the main function in `main.rs` to route the command
4. Add tests for the new command
5. Update documentation

## Performance Considerations

- Minimize allocations in hot paths
- Use efficient data structures
- Consider async operations where appropriate
- Profile performance-critical sections
- Avoid unnecessary system calls

## Security Considerations

- Validate all user inputs
- Sanitize application names and other user-provided strings
- Use secure defaults
- Prevent directory traversal attacks
- Limit resource usage where possible

## Pull Request Guidelines

1. Describe your changes in the PR description
2. Reference any related issues
3. Include tests for new functionality
4. Update documentation as needed
5. Ensure all CI checks pass
6. Be responsive to feedback during review

## Getting Help

- Open an issue for bug reports or feature requests
- Join the discussion in existing issues
- Contact maintainers if you need guidance

## Code of Conduct

Please follow the project's Code of Conduct in all interactions.

Thank you for contributing to Riku!