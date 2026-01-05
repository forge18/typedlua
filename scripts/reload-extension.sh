#!/bin/bash
# Quick reload script for VS Code extension only (skips LSP rebuild)
# Use this when you only changed extension TypeScript code

set -e

echo "⚡ TypedLua: Quick Extension Reload"
echo "===================================="
echo ""

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT/editors/vscode"

echo "🔨 Compiling extension..."
npm run compile
echo ""

echo "📦 Packaging..."
mkdir -p dist
rm -f dist/typedlua-*.vsix
npx vsce package --allow-missing-repository --no-dependencies --out dist/ 2>&1 | grep -v "DeprecationWarning" || true
VSIX_FILE=$(ls dist/typedlua-*.vsix 2>/dev/null | head -1)

if [ -z "$VSIX_FILE" ]; then
    echo "❌ Error: Failed to create .vsix file"
    exit 1
fi
echo ""

echo "📥 Installing..."
code --install-extension "$VSIX_FILE" --force
echo ""

echo "✅ Done! Reload VS Code window (Ctrl+Shift+P > Reload Window)"
