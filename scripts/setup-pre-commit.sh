#!/bin/bash
#
# Setup script for pre-commit and pre-push hooks
#

set -e

echo "Setting up pre-commit and pre-push hooks for SQLx project..."

# Check if pre-commit is installed
if ! command -v pre-commit &> /dev/null; then
    echo "pre-commit is not installed. Installing..."
    
    # Try different installation methods
    if command -v pip &> /dev/null; then
        pip install pre-commit
    elif command -v pip3 &> /dev/null; then
        pip3 install pre-commit
    elif command -v brew &> /dev/null; then
        brew install pre-commit
    else
        echo "Please install pre-commit manually:"
        echo "  pip install pre-commit"
        echo "  or"
        echo "  brew install pre-commit"
        exit 1
    fi
fi

# Install the git hook scripts
echo "Installing pre-commit and pre-push git hooks..."
pre-commit install --hook-type pre-commit
pre-commit install --hook-type pre-push

echo "Testing pre-commit setup..."
pre-commit run --all-files --hook-stage commit || true

echo ""
echo "✅ Pre-commit and pre-push hooks installed successfully!"
echo ""
echo "The following hooks are now active:"
echo ""
echo "📝 On commit (fast checks):"
echo "  - Rust code formatting check"
echo "  - Trailing whitespace removal"
echo "  - End-of-file fixing"
echo "  - YAML and TOML validation"
echo ""
echo "🚀 On push (comprehensive checks):"
echo "  - SQLx query cache validation"
echo "  - Rust clippy linting"
echo ""
echo "To run manually:"
echo "  pre-commit run --all-files --hook-stage commit"
echo "  pre-commit run --all-files --hook-stage push"
echo ""
echo "To skip hooks:"
echo "  git commit --no-verify"
echo "  git push --no-verify"