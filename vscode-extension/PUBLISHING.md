# VS Code Marketplace Publishing Checklist

## Pre-Publishing Requirements

### 1. Extension Manifest (package.json) ✅
- [x] Name: `regent`
- [x] Display Name: `Regent - OpenVox Development Kit`
- [x] Description: Clear and concise
- [x] Version: `0.1.0`
- [x] Publisher: Set to your publisher ID
- [x] Icon: `icon.png` (128x128, included)
- [x] Categories: Appropriate categories selected
- [x] Keywords: Relevant search terms
- [x] License: AGPL-3.0 (included)
- [x] Repository: GitHub URL
- [x] Engines: VS Code version specified

### 2. Documentation ✅
- [x] README.md with:
  - Clear feature description
  - Installation requirements
  - Usage instructions
  - Configuration settings
  - Screenshots/GIFs (recommended)
- [x] EXAMPLES.md with practical workflows
- [x] LICENSE file included
- [x] CHANGELOG.md (create if needed)

### 3. Code Quality ✅
- [x] Extension compiles without errors: `npm run compile`
- [x] No TypeScript errors
- [x] ESLint configured
- [x] Tests written and passing: `npm test`
- [x] All commands registered and working
- [x] Error handling implemented

### 4. Functionality Testing ✅
- [x] All commands work correctly
- [x] Status bar integration functional
- [x] Diagnostics display properly
- [x] Code actions work
- [x] Snippets load correctly
- [x] Configuration settings apply
- [x] Extension activates on workspace with metadata.json

### 5. Assets
- [x] Icon (icon.png): 128x128 pixels, PNG format
- [ ] Screenshots: Add to README.md (optional but recommended)
- [ ] Demo GIF: Show extension in action (optional but recommended)

## Publishing Steps

### Step 1: Create Publisher Account
```bash
# If you don't have a publisher account yet:
# 1. Go to https://marketplace.visualstudio.com/manage
# 2. Sign in with Microsoft/GitHub account
# 3. Create a new publisher
```

### Step 2: Get Personal Access Token
```bash
# 1. Go to https://dev.azure.com/<your-org>/_usersSettings/tokens
# 2. Create new token with "Marketplace" scope
# 3. Select "Marketplace: Manage" permission
# 4. Copy the token (you won't see it again!)
```

### Step 3: Login to vsce
```bash
cd vscode-extension
npm install -g @vscode/vsce
vsce login <your-publisher-id>
# Enter your Personal Access Token when prompted
```

### Step 4: Update package.json Publisher
```json
{
  "publisher": "your-publisher-id-here"
}
```

### Step 5: Build and Package
```bash
# From regent root:
make vscode-extension

# Or manually:
cd vscode-extension
npm install
npm run compile
vsce package
```

This creates `regent-0.1.0.vsix`

### Step 6: Test the VSIX Locally
```bash
# Install locally to test
code --install-extension regent-0.1.0.vsix

# Test all functionality
# Uninstall when done:
code --uninstall-extension regent.regent
```

### Step 7: Publish to Marketplace
```bash
vsce publish
# OR specify version bump:
# vsce publish patch  # 0.1.0 -> 0.1.1
# vsce publish minor  # 0.1.0 -> 0.2.0
# vsce publish major  # 0.1.0 -> 1.0.0
```

### Step 8: Verify Publication
1. Visit https://marketplace.visualstudio.com/items?itemName=<publisher>.<extension-name>
2. Check that all information displays correctly
3. Test installation from marketplace:
   ```bash
   code --install-extension <publisher>.regent
   ```

## Post-Publishing

### Update README Badge (Optional)
Add marketplace badge to README:
```markdown
[![VS Code Marketplace](https://img.shields.io/vscode-marketplace/v/<publisher>.regent.svg)](https://marketplace.visualstudio.com/items?itemName=<publisher>.regent)
```

### Create GitHub Release
1. Tag the release: `git tag v0.1.0`
2. Push tag: `git push origin v0.1.0`
3. Create GitHub release with VSIX attached

### Monitor
- Check ratings and reviews
- Respond to issues on GitHub
- Plan next version features

## Version Updates

For subsequent releases:

1. Update CHANGELOG.md with new features/fixes
2. Bump version in package.json
3. Commit changes
4. Run: `vsce publish patch|minor|major`

## Troubleshooting

### "Missing publisher"
- Set `publisher` field in package.json

### "Invalid icon"
- Ensure icon.png is exactly 128x128 pixels
- Use PNG format only

### "Package too large"
- Check .vscodeignore excludes node_modules, out/, etc.
- Verify: `vsce ls` (shows what will be packaged)

### "Invalid extension"
- Validate package.json with: `vsce package --list-files`
- Ensure all required fields present

## Marketplace Guidelines

Ensure compliance with:
- [VS Code Extension Guidelines](https://code.visualstudio.com/api/references/extension-guidelines)
- [Marketplace Terms of Use](https://aka.ms/vsmarketplace-ToU)
- No telemetry without user consent (✅ we don't use any)
- Respect user privacy

## Pre-Flight Checklist Summary

Before running `vsce publish`:

- [ ] Set correct publisher ID in package.json
- [ ] Version number updated appropriately
- [ ] CHANGELOG.md updated
- [ ] All tests passing
- [ ] Extension tested locally via VSIX
- [ ] README has screenshots/examples
- [ ] No console.log statements in production code
- [ ] Error handling robust
- [ ] Configuration documented
- [ ] License file included

## Ready to Publish?

```bash
cd vscode-extension
vsce publish
```

🎉 Your extension is now live on the marketplace!
