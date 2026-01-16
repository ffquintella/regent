# Regent VS Code Extension - Examples

## Getting Started

### 1. Install Regent

First, ensure Regent is installed and available in your PATH:

```bash
# macOS/Linux
which regent

# Windows
where regent
```

If not installed, refer to the [Regent installation guide](https://github.com/ffquintella/regent).

### 2. Open a Puppet Module

Open any Puppet module directory in VS Code. The extension will activate automatically if it detects a `metadata.json` file.

## Common Workflows

### Building a Module

1. Open Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`)
2. Type "Regent: Build Module"
3. Or click the Regent icon in the status bar and select "Build Module"

The module will be packaged and saved in the `pkg/` directory.

### Running Tests

**Quick Method:**
- Press `Cmd+Shift+P` / `Ctrl+Shift+P`
- Select "Regent: Run Tests"

**Using Tasks:**
- Press `Cmd+Shift+B` / `Ctrl+Shift+B`
- Select "Regent: Run Tests" (default test task)

### Linting Your Code

**Manual Lint:**
```
Cmd+Shift+P -> Regent: Lint Module
```

**Automatic Lint on Save:**
1. Open Settings (`Cmd+,` / `Ctrl+,`)
2. Search for "regent.lintOnSave"
3. Enable the checkbox

**View Issues:**
- Lint issues appear in the Problems panel (`Cmd+Shift+M` / `Ctrl+Shift+M`)
- Inline squiggly lines in your code

**Fix Issues:**
1. Click on a lint warning in Problems panel
2. Click the lightbulb icon or press `Cmd+.` / `Ctrl+.`
3. Select "Fix All Auto-fixable Issues"

### Generating Components

**Generate a Class:**
```
Cmd+Shift+P -> Regent: Generate Component
Select: class
Enter name: apache::config
```

**Generate a Defined Type:**
```
Cmd+Shift+P -> Regent: Generate Component
Select: defined-type
Enter name: apache::vhost
```

## Snippets

Type these prefixes in `.pp` files and press Tab:

### Classes and Types
- `regent-class` - Full class with documentation
- `regent-define` - Defined type with documentation

### Resources
- `regent-file` - File resource with common attributes
- `regent-package` - Package resource
- `regent-service` - Service resource  
- `regent-exec` - Exec resource with conditions

### Testing (in spec files)
- `regent-describe` - RSpec describe block with OS iteration
- `regent-context` - Context block with params
- `regent-it` - Expectation block

### Example: Using the Class Snippet

1. Create a new `.pp` file
2. Type `regent-class` and press Tab
3. Fill in the placeholders:
   - Description
   - Module name
   - Class name
   - Parameters

Result:
```puppet
# @summary Brief description of class
#
# Longer description of class
#
# @param param_name
#   Description of parameter
#
# @example
#   include mymodule::myclass
#
class mymodule::myclass (
  String $param_name = 'default',
) {
  # Your code here
}
```

## Configuration

### Custom Binary Path

If Regent is not in your PATH:

```json
{
  "regent.binaryPath": "/usr/local/bin/regent"
}
```

### Lint on Save

```json
{
  "regent.lintOnSave": true
}
```

### Fail on Warnings

Treat warnings as errors:

```json
{
  "regent.failOnWarnings": true
}
```

### Disable Diagnostics

Turn off the Problems panel integration:

```json
{
  "regent.enableDiagnostics": false
}
```

## Keyboard Shortcuts

Add custom keyboard shortcuts in `keybindings.json`:

```json
[
  {
    "key": "ctrl+shift+b",
    "command": "regent.build"
  },
  {
    "key": "ctrl+shift+t",
    "command": "regent.test"
  },
  {
    "key": "ctrl+shift+l",
    "command": "regent.lint"
  }
]
```

## Workspace Setup

For new projects, run:

```
Cmd+Shift+P -> Regent: Setup Workspace
```

This creates:
- `.vscode/tasks.json` - Build, test, and lint tasks
- `.vscode/settings.json` - Regent configuration

## Troubleshooting

### "Regent binary not found"

1. Verify Regent is installed: `regent --version`
2. Check your PATH includes the Regent binary
3. Set custom path: Settings → `regent.binaryPath`

### Lint Diagnostics Not Showing

1. Check Settings → `regent.enableDiagnostics` is enabled
2. Ensure `metadata.json` exists in workspace root
3. Run lint manually: `Cmd+Shift+P` → "Regent: Lint Module"

### Commands Not Working

1. Ensure workspace folder is open (File → Open Folder)
2. Check Output panel for errors (View → Output → Regent)
3. Reload window: `Cmd+Shift+P` → "Developer: Reload Window"

## Advanced

### Custom Tasks

Edit `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Regent: Build and Test",
      "type": "shell",
      "command": "regent build && regent test",
      "problemMatcher": ["$regent-lint", "$regent-test"]
    }
  ]
}
```

### Integration with CI/CD

Use the same Regent commands in your CI pipeline:

```yaml
# .github/workflows/test.yml
- name: Lint
  run: regent lint --fail-on-warnings
  
- name: Test
  run: regent test
  
- name: Build
  run: regent build
```
