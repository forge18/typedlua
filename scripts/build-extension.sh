#!/bin/bash
# Build TypedLua VS Code extension without installing
# Creates the .vsix package for distribution or manual installation

set -e

echo "🚀 TypedLua: Build Extension Package"
echo "====================================="
echo ""

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

# Step 1: Build the LSP server
echo "📦 Step 1/3: Building LSP server..."
cargo build --release --package typedlua-lsp
echo "✅ LSP server built: target/release/typedlua-lsp"
echo ""

# Step 2: Compile the extension
echo "🔨 Step 2/3: Compiling VS Code extension..."
cd editors/vscode
npm run compile
echo "✅ Extension compiled"
echo ""

# Step 3: Package the extension
echo "📦 Step 3/3: Packaging extension as VSIX..."
rm -f typedlua-*.vsix
npx vsce package --allow-missing-repository --no-dependencies 2>&1 | grep -v "DeprecationWarning" || true
VSIX_FILE=$(ls typedlua-*.vsix 2>/dev/null | head -1)

if [ -z "$VSIX_FILE" ]; then
    echo "❌ Error: Failed to create .vsix file"
    exit 1
fi

echo ""
echo "====================================="
echo "✅ Build complete!"
echo ""
echo "📦 Package created: editors/vscode/$VSIX_FILE"
echo ""
echo "To install manually:"
echo "  code --install-extension editors/vscode/$VSIX_FILE"
echo ""
echo "Or use the quick install script:"
echo "  ./rebuild-and-install-extension.sh"
echo ""
echo "====================================="
