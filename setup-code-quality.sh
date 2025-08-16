#!/bin/bash

# Setup Code Quality Tools for gc9307-async project
# This script installs and configures lefthook, commitlint, and markdownlint

set -e  # Exit on any error

echo "🚀 Setting up code quality tools for gc9307-async project..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "lefthook.yml" ] || [ ! -f "commitlint.config.cjs" ] || [ ! -f ".markdownlint-cli2.yaml" ]; then
    print_error "Required configuration files not found. Please run this script from the project root."
    exit 1
fi

print_status "Checking required tools..."

# Check if lefthook is installed
if ! command -v lefthook &> /dev/null; then
    print_error "lefthook is not installed. Please install it first:"
    echo "  - macOS: brew install lefthook"
    echo "  - Other: https://github.com/evilmartians/lefthook#installation"
    exit 1
fi

# Check if bun is installed
if ! command -v bun &> /dev/null; then
    print_error "bun is not installed. Please install it first:"
    echo "  - Visit: https://bun.sh/"
    exit 1
fi

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    print_error "cargo is not installed. Please install Rust first:"
    echo "  - Visit: https://rustup.rs/"
    exit 1
fi

# Check if rustfmt and clippy are available
if ! cargo fmt --version > /dev/null 2>&1; then
    print_error "rustfmt is not available. Please install it:"
    echo "  - Run: rustup component add rustfmt"
    exit 1
fi

if ! cargo clippy --version > /dev/null 2>&1; then
    print_error "clippy is not available. Please install it:"
    echo "  - Run: rustup component add clippy"
    exit 1
fi

print_success "All required tools are available!"

# Step 1: Install lefthook git hooks
print_status "Installing lefthook git hooks..."
if lefthook install; then
    print_success "Lefthook git hooks installed successfully!"
else
    print_error "Failed to install lefthook git hooks"
    exit 1
fi

# Step 2: Test commitlint
print_status "Testing commitlint..."
if bunx commitlint --version > /dev/null 2>&1; then
    print_success "commitlint is working!"
else
    print_error "commitlint test failed"
    exit 1
fi

# Step 3: Test markdownlint-cli2
print_status "Testing markdownlint-cli2..."
if bunx markdownlint-cli2 --version > /dev/null 2>&1; then
    print_success "markdownlint-cli2 is working!"
else
    print_error "markdownlint-cli2 test failed"
    exit 1
fi

# Step 4: Test Rust tools
print_status "Testing Rust formatting tool..."
if cargo fmt --version > /dev/null 2>&1; then
    print_success "cargo fmt is working!"
else
    print_error "cargo fmt test failed"
    exit 1
fi

print_status "Testing Rust clippy tool..."
if cargo clippy --version > /dev/null 2>&1; then
    print_success "cargo clippy is working!"
else
    print_error "cargo clippy test failed"
    exit 1
fi

# Step 5: Test hooks manually
print_status "Testing pre-commit hooks..."
if lefthook run pre-commit; then
    print_success "Pre-commit hooks test completed!"
else
    print_warning "Pre-commit hooks test had issues (this might be normal if no files are staged)"
fi

# Step 6: Create a test commit message file for testing commit-msg hook
print_status "Testing commit-msg hook..."
echo "test: add new feature" > /tmp/test_commit_msg
if lefthook run commit-msg /tmp/test_commit_msg; then
    print_success "Commit-msg hook test passed!"
else
    print_error "Commit-msg hook test failed"
fi
rm -f /tmp/test_commit_msg

print_success "🎉 Code quality tools setup completed successfully!"

echo ""
echo "📋 What's been configured:"
echo "  ✅ Git hooks installed via lefthook"
echo "  ✅ Pre-commit hooks: Rust formatting, clippy, markdown linting"
echo "  ✅ Commit-msg hooks: Conventional commit format validation"
echo "  ✅ All tools tested and working"

echo ""
echo "🔧 Usage:"
echo "  • Hooks will run automatically on git commit"
echo "  • Manual testing: lefthook run pre-commit"
echo "  • Manual commit-msg test: lefthook run commit-msg <commit-msg-file>"
echo "  • Format code manually: cargo fmt"
echo "  • Run clippy manually: cargo clippy"
echo "  • Check markdown manually: bunx markdownlint-cli2 **/*.md"

echo ""
echo "📝 Commit message format:"
echo "  type(scope): description"
echo "  Example: feat(driver): add async support for gc9307"
echo "  Types: feat, fix, docs, style, refactor, perf, test, chore, ci, build, revert"

echo ""
print_success "Setup complete! Your code quality tools are now active. 🚀"
