#!/bin/bash
# Complete rebuild and installation script for TypedLua VS Code extension
# Run this from the project root whenever you make changes

set -e  # Exit on any error

echo "🚀 TypedLua: Complete Rebuild and Install"
echo "=========================================="
echo ""

# Get the project root (one level up from scripts directory)
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

# Step 1: Build the LSP server
echo "📦 Step 1/4: Building LSP server..."
cargo build --release --package typedlua-lsp
echo "✅ LSP server built: target/release/typedlua-lsp"
echo ""

# Step 2: Compile the extension
echo "🔨 Step 2/4: Compiling VS Code extension..."
cd editors/vscode
npm run compile
echo "✅ Extension compiled"
echo ""

# Step 3: Package the extension
echo "📦 Step 3/4: Packaging extension as VSIX..."
# Create dist directory
mkdir -p dist
rm -f dist/typedlua-*.vsix
npx vsce package --allow-missing-repository --no-dependencies --out dist/ 2>&1 | grep -v "DeprecationWarning" || true
VSIX_FILE=$(ls dist/typedlua-*.vsix 2>/dev/null | head -1)

if [ -z "$VSIX_FILE" ]; then
    echo "❌ Error: Failed to create .vsix file"
    exit 1
fi
echo "✅ Created: $VSIX_FILE"
echo ""

# Step 4: Install the extension
echo "📥 Step 4/4: Installing extension in VS Code..."
code --install-extension "$VSIX_FILE" --force
echo "✅ Extension installed!"
echo ""

echo "=========================================="
echo "✨ All done! Next steps:"
echo ""
echo "1. Reload VS Code:"
echo "   - Press Ctrl+Shift+P (Cmd+Shift+P on Mac)"
echo "   - Type 'Reload Window' and press Enter"
echo ""
echo "2. Open a .tl file to test:"
echo "   code editors/vscode/test-files/test-basic.tl"
echo ""
echo "3. Check Output panel for LSP logs:"
echo "   - View > Output"
echo "   - Select 'TypedLua Language Server'"
echo ""
echo "=========================================="
