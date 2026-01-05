# Publishing to VS Code Marketplace

This guide walks you through publishing the TypedLua extension to the Visual Studio Code Marketplace.

## Prerequisites

### 1. Create a Microsoft/Azure Account

1. Go to [https://azure.microsoft.com](https://azure.microsoft.com)
2. Sign up for a free account (if you don't have one)
3. You can use a personal Microsoft account or create a new one

### 2. Create an Azure DevOps Organization

1. Go to [https://dev.azure.com](https://dev.azure.com)
2. Sign in with your Microsoft account
3. Click "Create new organization"
4. Choose a unique name for your organization (e.g., "typedlua")
5. Select your region
6. Complete the setup

### 3. Create a Personal Access Token (PAT)

1. In Azure DevOps, click your profile icon (top right)
2. Select "Personal access tokens"
3. Click "+ New Token"
4. Configure the token:
   - **Name**: "VS Code Marketplace" (or similar)
   - **Organization**: Select "All accessible organizations"
   - **Expiration**: Choose duration (max 1 year, recommended: 90 days)
   - **Scopes**: Select "Custom defined"
   - Check **"Marketplace" > "Manage"** (this is crucial!)
5. Click "Create"
6. **IMPORTANT**: Copy the token immediately and save it securely
   - You won't be able to see it again
   - Store it in a password manager

### 4. Create a Publisher

1. Go to [https://marketplace.visualstudio.com/manage](https://marketplace.visualstudio.com/manage)
2. Sign in with the same Microsoft account
3. Click "Create publisher"
4. Fill in the form:
   - **ID**: Unique identifier (e.g., "typedlua-team") - can't be changed later
   - **Name**: Display name (e.g., "TypedLua")
   - **Email**: Contact email
   - **Logo**: Optional (can add later)
5. Click "Create"

### 5. Install vsce (if not already installed)

```bash
npm install -g @vscode/vsce
```

## Pre-Publishing Checklist

Before publishing, ensure:

- [ ] Extension is fully tested
- [ ] README.md is complete and professional
- [ ] CHANGELOG.md is up to date
- [ ] package.json has correct information:
  - [ ] `publisher` field matches your publisher ID
  - [ ] `version` is correct
  - [ ] `repository` URL is correct
  - [ ] `icon` is set and looks good
  - [ ] `keywords` are relevant
- [ ] All features work as expected
- [ ] No sensitive information in code
- [ ] LICENSE file exists (MIT, Apache 2.0, etc.)

## Publishing Steps

### 1. Update package.json

Make sure the `publisher` field matches your publisher ID:

```json
{
  "publisher": "your-publisher-id",
  "version": "0.1.0",
  ...
}
```

### 2. Login to vsce

```bash
vsce login your-publisher-id
```

When prompted, paste your Personal Access Token (PAT).

### 3. Package the Extension

From the extension directory:

```bash
cd editors/vscode
vsce package
```

This creates `typedlua-0.1.0.vsix` (or similar).

**Optional**: Test the .vsix locally before publishing:
```bash
code --install-extension typedlua-0.1.0.vsix
```

### 4. Publish to Marketplace

```bash
vsce publish
```

This will:
1. Package the extension
2. Upload to the marketplace
3. Make it available to users (after validation, usually 5-10 minutes)

**Alternative**: Publish a specific version:
```bash
vsce publish 0.1.0
```

**Alternative**: Publish and increment version:
```bash
vsce publish patch  # 0.1.0 -> 0.1.1
vsce publish minor  # 0.1.0 -> 0.2.0
vsce publish major  # 0.1.0 -> 1.0.0
```

### 5. Verify Publication

1. Go to [https://marketplace.visualstudio.com/items?itemName=your-publisher-id.typedlua](https://marketplace.visualstudio.com/items?itemName=your-publisher-id.typedlua)
2. Check that all information is correct
3. Try installing in VS Code:
   ```
   Ctrl+Shift+X > Search "TypedLua" > Install
   ```

## Updating the Extension

When you make changes:

### 1. Update Version

In `package.json`, increment the version:
```json
{
  "version": "0.1.1"  // or 0.2.0, 1.0.0, etc.
}
```

### 2. Update CHANGELOG.md

Add new version section:
```markdown
## [0.1.1] - 2026-01-10

### Fixed
- Bug fix description

### Added
- New feature description
```

### 3. Publish Update

```bash
vsce publish
```

Or let vsce increment the version:
```bash
vsce publish patch  # For bug fixes
vsce publish minor  # For new features
vsce publish major  # For breaking changes
```

## Automated Publishing with Scripts

### Option 1: Using Our Build Script

```bash
# From project root
./build-extension.sh

# Then publish
cd editors/vscode
vsce publish
```

### Option 2: Create a Publish Script

Create `editors/vscode/publish.sh`:

```bash
#!/bin/bash
set -e

echo "🚀 Publishing TypedLua Extension"
echo "================================="

# Ensure we're in the right directory
cd "$(dirname "$0")"

# Run tests (if you have them)
# npm test

# Lint
npm run lint

# Compile
npm run compile

# Publish
vsce publish

echo "✅ Published successfully!"
```

Make it executable:
```bash
chmod +x editors/vscode/publish.sh
```

Use it:
```bash
./editors/vscode/publish.sh
```

## Managing Your Extension

### View Statistics

1. Go to [https://marketplace.visualstudio.com/manage](https://marketplace.visualstudio.com/manage)
2. Sign in
3. Click on your extension
4. View:
   - Install count
   - Ratings and reviews
   - Download statistics

### Update Extension Details

1. Go to marketplace management page
2. Click on your extension
3. You can update:
   - Description
   - Logo
   - Categories
   - Tags
   - Screenshots
   - Q&A settings

### Respond to Reviews

- Users can leave reviews and ratings
- Respond professionally to feedback
- Address issues and bugs mentioned in reviews

## Unpublishing

**Warning**: Only unpublish if absolutely necessary. Users with the extension installed will be affected.

```bash
vsce unpublish your-publisher-id.typedlua
```

## Best Practices

### Versioning Strategy

Follow [Semantic Versioning](https://semver.org/):
- **Patch** (0.1.0 → 0.1.1): Bug fixes, no new features
- **Minor** (0.1.0 → 0.2.0): New features, backward compatible
- **Major** (0.1.0 → 1.0.0): Breaking changes

### Release Process

1. **Test thoroughly** on multiple systems
2. **Update documentation** (README, CHANGELOG)
3. **Tag the release** in git:
   ```bash
   git tag v0.1.0
   git push --tags
   ```
4. **Publish** to marketplace
5. **Announce** on social media, GitHub, etc.

### Quality Checklist

Before each release:
- [ ] All tests pass
- [ ] No console errors or warnings
- [ ] Extension activates properly
- [ ] All features work as documented
- [ ] README is up to date
- [ ] CHANGELOG is updated
- [ ] Version number is incremented
- [ ] Screenshots are current (if applicable)

## Troubleshooting

### "Publisher not found"

- Verify you created a publisher on [marketplace.visualstudio.com/manage](https://marketplace.visualstudio.com/manage)
- Check the publisher ID matches exactly in package.json

### "Invalid Personal Access Token"

- Make sure the PAT has "Marketplace > Manage" scope
- Check if the PAT has expired
- Create a new PAT if needed

### "Extension validation failed"

- Check package.json for errors
- Ensure all required fields are present
- Make sure LICENSE file exists
- Check that icon file exists and is referenced correctly

### "Cannot find module"

- Run `npm install` to ensure all dependencies are installed
- Check that `node_modules` is not in `.vscodeignore`

## Resources

- [VS Code Publishing Guide](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [vsce Documentation](https://github.com/microsoft/vscode-vsce)
- [Marketplace Management](https://marketplace.visualstudio.com/manage)
- [Azure DevOps](https://dev.azure.com)

## Support

If you encounter issues:
1. Check the [vsce GitHub Issues](https://github.com/microsoft/vscode-vsce/issues)
2. Ask on [VS Code Extension Development Discord](https://discord.gg/vscode-extension-development)
3. Review the [official documentation](https://code.visualstudio.com/api)

---

**Good luck with your extension!** 🚀
