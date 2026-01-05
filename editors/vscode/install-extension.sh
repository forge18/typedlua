#!/bin/bash
# Script to package and install TypedLua VS Code extension locally

set -e

echo "📦 Building TypedLua VS Code Extension..."

# Ensure we're in the right directory
cd "$(dirname "$0")"

# Compile TypeScript
echo "🔨 Compiling TypeScript..."
npm run compile

# Package the extension
echo "📦 Packaging extension..."
npx vsce package --allow-missing-repository

# Find the .vsix file
VSIX_FILE=$(ls typedlua-*.vsix | head -1)

if [ -z "$VSIX_FILE" ]; then
    echo "❌ Error: .vsix file not found"
    exit 1
fi

echo "✅ Created: $VSIX_FILE"

# Install the extension
echo "📥 Installing extension..."
code --install-extension "$VSIX_FILE" --force

echo "✅ Extension installed successfully!"
echo ""
echo "To use:"
echo "1. Reload VS Code window"
echo "2. Open or create a .tl file"
echo "3. The TypedLua extension will activate"
echo ""
echo "To test with samples:"
echo "  code test-files/"
