#!/bin/bash
# Comprehensive test runner for Riku
# Runs all tests: unit, integration, and deployment tests

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

print_header() {
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
}

# Run Rust unit tests
run_unit_tests() {
    print_header "Running Rust Unit Tests"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo test --bin riku 2>&1 | grep -E "(running|test result)"; then
        log_success "Rust unit tests passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Rust unit tests failed"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Run Rust integration tests
run_integration_tests() {
    print_header "Running Rust Integration Tests"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo test --test '*' 2>&1; then
        log_success "Rust integration tests passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Rust integration tests failed"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Run all Rust tests together
run_rust_tests() {
    print_header "Running All Rust Tests"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo test 2>&1 | grep -E "(running|test result)"; then
        log_success "All Rust tests passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Rust tests failed"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Run shell deployment tests
run_deployment_tests() {
    print_header "Running Deployment Tests"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if [ -f "tests/deploy/test-all.sh" ]; then
        if bash tests/deploy/test-all.sh 2>&1; then
            log_success "Deployment tests passed"
            PASSED_TESTS=$((PASSED_TESTS + 1))
        else
            log_error "Deployment tests failed"
            FAILED_TESTS=$((FAILED_TESTS + 1))
            return 1
        fi
    else
        log_warning "Deployment test script not found"
    fi
}

# Run clippy lints
run_clippy() {
    print_header "Running Clippy Lints"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo clippy -- -D warnings 2>&1; then
        log_success "Clippy lints passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Clippy lints failed"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Run cargo fmt check
run_fmt_check() {
    print_header "Running Format Check"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo fmt -- --check 2>&1; then
        log_success "Format check passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Format check failed"
        log_info "Run 'cargo fmt' to fix formatting issues"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Build release binary
run_build() {
    print_header "Building Release Binary"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if cargo build --release 2>&1; then
        log_success "Release build succeeded"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        log_error "Release build failed"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# Print summary
print_summary() {
    print_header "Test Summary"

    echo -e "Total test suites:  ${TOTAL_TESTS}"
    echo -e "${GREEN}Passed:             ${PASSED_TESTS}${NC}"
    echo -e "${RED}Failed:             ${FAILED_TESTS}${NC}"
    echo ""

    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}✗ Some tests failed!${NC}"
        echo ""
        return 1
    fi
}

# Show help
show_help() {
    echo "Riku Test Runner"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --unit          Run only unit tests"
    echo "  --integration   Run only integration tests"
    echo "  --deployment    Run only deployment tests"
    echo "  --clippy        Run clippy lints"
    echo "  --fmt           Run format check"
    echo "  --build         Run release build"
    echo "  --all           Run all tests (default)"
    echo "  --quick         Run quick tests only (unit + integration)"
    echo "  -h, --help      Show this help message"
    echo ""
}

# Main
main() {
    local run_all=true
    local run_unit=false
    local run_integration=false
    local run_deployment=false
    local run_clippy=false
    local run_fmt=false
    local run_build=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --unit)
                run_unit=true
                run_all=false
                shift
                ;;
            --integration)
                run_integration=true
                run_all=false
                shift
                ;;
            --deployment)
                run_deployment=true
                run_all=false
                shift
                ;;
            --clippy)
                run_clippy=true
                run_all=false
                shift
                ;;
            --fmt)
                run_fmt=true
                run_all=false
                shift
                ;;
            --build)
                run_build=true
                run_all=false
                shift
                ;;
            --all)
                run_all=true
                shift
                ;;
            --quick)
                run_all=false
                run_unit=true
                run_integration=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done

    # Change to project root
    cd "$(dirname "$0")/.."

    print_header "Riku Test Suite"
    echo "Starting test run..."
    echo ""

    if [ "$run_all" = true ]; then
        run_rust_tests
        run_deployment_tests
        run_clippy
        run_fmt
        run_build
    else
        if [ "$run_unit" = true ]; then
            run_unit_tests
        fi

        if [ "$run_integration" = true ]; then
            run_integration_tests
        fi

        if [ "$run_deployment" = true ]; then
            run_deployment_tests
        fi

        if [ "$run_clippy" = true ]; then
            run_clippy
        fi

        if [ "$run_fmt" = true ]; then
            run_fmt_check
        fi

        if [ "$run_build" = true ]; then
            run_build
        fi
    fi

    print_summary
    exit_code=$?

    exit $exit_code
}

main "$@"
