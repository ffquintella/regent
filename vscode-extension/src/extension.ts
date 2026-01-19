import * as vscode from 'vscode';
import * as path from 'path';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

let diagnosticCollection: vscode.DiagnosticCollection;
let outputChannel: vscode.OutputChannel;
let statusBarItem: vscode.StatusBarItem;
let lintCache = new Map<string, { timestamp: number, diagnostics: vscode.Diagnostic[] }>();
const CACHE_TTL = 30000; // 30 seconds cache

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('Regent');
    diagnosticCollection = vscode.languages.createDiagnosticCollection('regent');
    context.subscriptions.push(diagnosticCollection);
    context.subscriptions.push(outputChannel);

    // Create status bar item
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.text = "$(regent) Regent";
    statusBarItem.tooltip = "Regent - OpenVox Development Kit";
    statusBarItem.command = 'regent.showMenu';
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('regent.showMenu', () => showQuickMenu())
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('regent.build', () => runRegentCommand('build'))
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('regent.test', () => runRegentCommand('test'))
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('regent.lint', () => runRegentLint())
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('regent.validators', () => runRegentCommand('validators'))
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('regent.generate', () => runGenerateCommand())
    );

    // Register fix all command
    context.subscriptions.push(
        vscode.commands.registerCommand('regent.fixAll', () => runFixAll())
    );

    // Register setup workspace command
    context.subscriptions.push(
        vscode.commands.registerCommand('regent.setupWorkspace', () => setupWorkspace())
    );

    // Register code action provider for quick fixes
    context.subscriptions.push(
        vscode.languages.registerCodeActionsProvider('puppet', new RegentCodeActionProvider(), {
            providedCodeActionKinds: RegentCodeActionProvider.providedCodeActionKinds
        })
    );

    // Lint on save if enabled
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((document) => {
            if (document.languageId === 'puppet') {
                const config = vscode.workspace.getConfiguration('regent');
                if (config.get<boolean>('lintOnSave')) {
                    runRegentLint(false);
                }
            }
        })
    );

    outputChannel.appendLine('Regent extension activated');
}

async function showQuickMenu() {
    const options = [
        { label: '$(package) Build Module', command: 'build' },
        { label: '$(beaker) Run Tests', command: 'test' },
        { label: '$(search-fuzzy) Lint Module', command: 'lint' },
        { label: '$(list-unordered) List Validators', command: 'validators' },
        { label: '$(file-add) Generate Component', command: 'generate' },
    ];

    const selected = await vscode.window.showQuickPick(options, {
        placeHolder: 'Select a Regent command'
    });

    if (selected) {
        if (selected.command === 'generate') {
            await runGenerateCommand();
        } else if (selected.command === 'lint') {
            await runRegentLint();
        } else {
            await runRegentCommand(selected.command);
        }
    }
}

export function deactivate() {
    if (diagnosticCollection) {
        diagnosticCollection.dispose();
    }
    if (outputChannel) {
        outputChannel.dispose();
    }
    if (statusBarItem) {
        statusBarItem.dispose();
    }
}

async function getRegentBinary(): Promise<string> {
    const config = vscode.workspace.getConfiguration('regent');
    const binaryPath = config.get<string>('binaryPath') || 'regent';
    
    // Validate binary exists (basic check)
    try {
        const { execAsync } = require('child_process');
        await execAsync(`which ${binaryPath}`, { timeout: 2000 });
    } catch (error) {
        vscode.window.showWarningMessage(
            `Regent binary '${binaryPath}' not found in PATH. Please check your configuration.`,
            'Open Settings'
        ).then(selection => {
            if (selection === 'Open Settings') {
                vscode.commands.executeCommand('workbench.action.openSettings', 'regent.binaryPath');
            }
        });
    }
    
    return binaryPath;
}

async function getWorkspaceRoot(): Promise<string | undefined> {
    if (vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders.length > 0) {
        return vscode.workspace.workspaceFolders[0].uri.fsPath;
    }
    return undefined;
}

async function runRegentCommand(command: string, args: string[] = []) {
    const workspaceRoot = await getWorkspaceRoot();
    if (!workspaceRoot) {
        vscode.window.showErrorMessage('No workspace folder open. Please open a Puppet module folder.');
        return;
    }

    const regent = await getRegentBinary();
    const fullCommand = `${regent} ${command} ${args.join(' ')}`;

    // Update status bar
    statusBarItem.text = `$(sync~spin) Regent: ${command}...`;

    outputChannel.show(true);
    outputChannel.appendLine(`\n$ ${fullCommand}`);
    outputChannel.appendLine('─'.repeat(80));

    try {
        const { stdout, stderr } = await execAsync(fullCommand, {
            cwd: workspaceRoot,
            maxBuffer: 10 * 1024 * 1024, // 10MB buffer
            timeout: 300000 // 5 minutes timeout
        });

        if (stdout) {
            outputChannel.appendLine(stdout);
        }
        if (stderr && stderr.length > 0) {
            outputChannel.appendLine('STDERR:');
            outputChannel.appendLine(stderr);
        }

        statusBarItem.text = `$(check) Regent`;
        vscode.window.showInformationMessage(`Regent ${command} completed successfully`);
    } catch (error: any) {
        statusBarItem.text = `$(error) Regent`;
        
        const errorMessage = error.message || 'Unknown error';
        outputChannel.appendLine(`\nError: ${errorMessage}`);
        
        if (error.stdout) {
            outputChannel.appendLine('\nOutput before error:');
            outputChannel.appendLine(error.stdout);
        }
        if (error.stderr) {
            outputChannel.appendLine('\nError output:');
            outputChannel.appendLine(error.stderr);
        }
        
        // Provide helpful error messages
        if (error.code === 'ENOENT') {
            vscode.window.showErrorMessage(
                `Regent command not found. Please ensure Regent is installed and in your PATH.`,
                'Open Settings'
            ).then(selection => {
                if (selection === 'Open Settings') {
                    vscode.commands.executeCommand('workbench.action.openSettings', 'regent.binaryPath');
                }
            });
        } else if (error.killed || error.signal === 'SIGTERM') {
            vscode.window.showErrorMessage(`Regent ${command} timed out or was killed`);
        } else {
            vscode.window.showErrorMessage(`Regent ${command} failed: ${errorMessage}`, 'View Output').then(selection => {
                if (selection === 'View Output') {
                    outputChannel.show();
                }
            });
        }
    } finally {
        // Reset status bar after 3 seconds
        setTimeout(() => {
            statusBarItem.text = "$(regent) Regent";
        }, 3000);
    }
}

async function runRegentLint(showOutput: boolean = true) {
    const workspaceRoot = await getWorkspaceRoot();
    if (!workspaceRoot) {
        vscode.window.showErrorMessage('No workspace folder open');
        return;
    }

    const config = vscode.workspace.getConfiguration('regent');
    const regent = await getRegentBinary();
    const failOnWarnings = config.get<boolean>('failOnWarnings') ? ' --fail-on-warnings' : '';
    const enableDiagnostics = config.get<boolean>('enableDiagnostics');
    
    const fullCommand = `${regent} lint --report json${failOnWarnings}`;

    // Update status bar
    statusBarItem.text = `$(sync~spin) Regent: linting...`;

    if (showOutput) {
        outputChannel.show(true);
        outputChannel.appendLine(`\n$ ${fullCommand}`);
        outputChannel.appendLine('─'.repeat(80));
    }

    try {
        const { stdout, stderr } = await execAsync(fullCommand, {
            cwd: workspaceRoot,
            maxBuffer: 10 * 1024 * 1024
        });

        if (showOutput && stdout) {
            outputChannel.appendLine(stdout);
        }

        // Parse JSON output and create diagnostics
        if (enableDiagnostics) {
            try {
                const lintReport = JSON.parse(stdout);
                parseLintResults(lintReport, workspaceRoot);
            } catch (parseError) {
                console.error('Failed to parse lint output:', parseError);
            }
        }

        statusBarItem.text = `$(check) Regent`;
        if (showOutput) {
            vscode.window.showInformationMessage('Regent lint completed');
        }
    } catch (error: any) {
        statusBarItem.text = `$(warning) Regent`;
        if (showOutput) {
            outputChannel.appendLine(`Error: ${error.message}`);
            if (error.stdout) {
                outputChannel.appendLine(error.stdout);
            }
        }

        // Still try to parse output for diagnostics
        if (enableDiagnostics && error.stdout) {
            try {
                const lintReport = JSON.parse(error.stdout);
                parseLintResults(lintReport, workspaceRoot);
            } catch (parseError) {
                // Ignore parse errors
            }
        }

        if (showOutput) {
            vscode.window.showWarningMessage(`Regent lint found issues`);
        }
    } finally {
        // Reset status bar after 3 seconds
        setTimeout(() => {
            statusBarItem.text = "$(regent) Regent";
        }, 3000);
    }
}

function parseLintResults(lintReport: any, workspaceRoot: string) {
    diagnosticCollection.clear();

    const diagnosticsMap = new Map<string, vscode.Diagnostic[]>();
    const cacheTimestamp = Date.now();

    if (lintReport.results && Array.isArray(lintReport.results)) {
        for (const result of lintReport.results) {
            if (result.issues && Array.isArray(result.issues)) {
                for (const issue of result.issues) {
                    const filePath = path.join(workspaceRoot, issue.file || '');
                    const uri = vscode.Uri.file(filePath);
                    
                    const line = Math.max(0, (issue.line || 1) - 1);
                    const column = Math.max(0, (issue.column || 1) - 1);
                    const endColumn = column + (issue.length || 100);
                    const range = new vscode.Range(line, column, line, endColumn);

                    let severity = vscode.DiagnosticSeverity.Warning;
                    if (issue.severity === 'error') {
                        severity = vscode.DiagnosticSeverity.Error;
                    } else if (issue.severity === 'info') {
                        severity = vscode.DiagnosticSeverity.Information;
                    }

                    const diagnostic = new vscode.Diagnostic(
                        range,
                        issue.message || 'Unknown issue',
                        severity
                    );
                    diagnostic.source = 'regent';
                    diagnostic.code = issue.check || result.tool;

                    if (!diagnosticsMap.has(filePath)) {
                        diagnosticsMap.set(filePath, []);
                    }
                    diagnosticsMap.get(filePath)!.push(diagnostic);
                }
            }
        }
    }

    // Apply all diagnostics and update cache
    for (const [filePath, diagnostics] of diagnosticsMap) {
        diagnosticCollection.set(vscode.Uri.file(filePath), diagnostics);
        lintCache.set(filePath, { timestamp: cacheTimestamp, diagnostics });
    }
    
    // Clean up old cache entries
    for (const [key, value] of lintCache.entries()) {
        if (cacheTimestamp - value.timestamp > CACHE_TTL) {
            lintCache.delete(key);
        }
    }
}

async function runGenerateCommand() {
    const componentType = await vscode.window.showQuickPick(
        ['class', 'defined-type', 'resource', 'provider', 'function', 'task', 'plan'],
        {
            placeHolder: 'Select component type to generate'
        }
    );

    if (!componentType) {
        return;
    }

    const componentName = await vscode.window.showInputBox({
        prompt: `Enter ${componentType} name`,
        placeHolder: 'my_component'
    });

    if (!componentName) {
        return;
    }

    const args = [componentType, componentName];
    await runRegentCommand('generate', args);
}

// Code Action Provider for quick fixes
class RegentCodeActionProvider implements vscode.CodeActionProvider {
    public static readonly providedCodeActionKinds = [
        vscode.CodeActionKind.QuickFix
    ];

    provideCodeActions(
        document: vscode.TextDocument,
        range: vscode.Range | vscode.Selection,
        context: vscode.CodeActionContext,
        token: vscode.CancellationToken
    ): vscode.CodeAction[] | undefined {
        const regentDiagnostics = context.diagnostics.filter(
            diagnostic => diagnostic.source === 'regent'
        );

        if (regentDiagnostics.length === 0) {
            return [];
        }

        const actions: vscode.CodeAction[] = [];

        // Add "Run Regent Lint" action
        const lintAction = new vscode.CodeAction(
            'Run Regent Lint',
            vscode.CodeActionKind.QuickFix
        );
        lintAction.command = {
            command: 'regent.lint',
            title: 'Run Regent Lint'
        };
        actions.push(lintAction);

        // Add "Fix All Auto-fixable Issues" action (if autofix is available)
        const fixAllAction = new vscode.CodeAction(
            'Fix All Auto-fixable Issues',
            vscode.CodeActionKind.QuickFix
        );
        fixAllAction.command = {
            command: 'regent.fixAll',
            title: 'Fix All Auto-fixable Issues'
        };
        fixAllAction.isPreferred = true;
        actions.push(fixAllAction);

        return actions;
    }
}

async function runFixAll() {
    const workspaceRoot = await getWorkspaceRoot();
    if (!workspaceRoot) {
        vscode.window.showErrorMessage('No workspace folder open');
        return;
    }

    const regent = await getRegentBinary();
    const fullCommand = `${regent} lint --fix`;

    statusBarItem.text = `$(sync~spin) Regent: fixing...`;
    outputChannel.show(true);
    outputChannel.appendLine(`\n$ ${fullCommand}`);
    outputChannel.appendLine('─'.repeat(80));

    try {
        const { stdout, stderr } = await execAsync(fullCommand, {
            cwd: workspaceRoot,
            maxBuffer: 10 * 1024 * 1024
        });

        if (stdout) {
            outputChannel.appendLine(stdout);
        }
        if (stderr) {
            outputChannel.appendLine(stderr);
        }

        statusBarItem.text = `$(check) Regent`;
        vscode.window.showInformationMessage('Auto-fixable issues resolved');
        
        // Re-run lint to update diagnostics
        setTimeout(() => runRegentLint(false), 500);
    } catch (error: any) {
        statusBarItem.text = `$(error) Regent`;
        outputChannel.appendLine(`Error: ${error.message}`);
        if (error.stdout) {
            outputChannel.appendLine(error.stdout);
        }
        if (error.stderr) {
            outputChannel.appendLine(error.stderr);
        }
        vscode.window.showErrorMessage(`Fix failed: ${error.message}`);
    } finally {
        setTimeout(() => {
            statusBarItem.text = "$(regent) Regent";
        }, 3000);
    }
}

async function setupWorkspace() {
    const workspaceRoot = await getWorkspaceRoot();
    if (!workspaceRoot) {
        vscode.window.showErrorMessage('No workspace folder open');
        return;
    }

    const vscodePath = path.join(workspaceRoot, '.vscode');
    const fs = require('fs').promises;

    try {
        // Create .vscode directory if it doesn't exist
        try {
            await fs.mkdir(vscodePath, { recursive: true });
        } catch (err) {
            // Directory might already exist
        }

        // Create tasks.json
        const tasksPath = path.join(vscodePath, 'tasks.json');
        const tasksContent = {
            version: '2.0.0',
            tasks: [
                {
                    label: 'Regent: Build Module',
                    type: 'shell',
                    command: 'regent',
                    args: ['build'],
                    problemMatcher: [],
                    group: {
                        kind: 'build',
                        isDefault: true
                    }
                },
                {
                    label: 'Regent: Run Tests',
                    type: 'shell',
                    command: 'regent',
                    args: ['test'],
                    problemMatcher: '$regent-test',
                    group: {
                        kind: 'test',
                        isDefault: true
                    }
                },
                {
                    label: 'Regent: Lint Module',
                    type: 'shell',
                    command: 'regent',
                    args: ['lint'],
                    problemMatcher: '$regent-lint'
                }
            ]
        };

        await fs.writeFile(tasksPath, JSON.stringify(tasksContent, null, 2));

        // Create settings.json with Regent configuration
        const settingsPath = path.join(vscodePath, 'settings.json');
        let settings: any = {};
        
        try {
            const existingSettings = await fs.readFile(settingsPath, 'utf8');
            settings = JSON.parse(existingSettings);
        } catch (err) {
            // File doesn't exist or can't be parsed, use empty object
        }

        // Add Regent settings if not present
        if (!settings['regent.enableDiagnostics']) {
            settings['regent.enableDiagnostics'] = true;
        }
        if (!settings['regent.lintOnSave']) {
            settings['regent.lintOnSave'] = false;
        }

        await fs.writeFile(settingsPath, JSON.stringify(settings, null, 2));

        vscode.window.showInformationMessage('Regent workspace setup complete! Created tasks.json and updated settings.json');
    } catch (error: any) {
        vscode.window.showErrorMessage(`Failed to setup workspace: ${error.message}`);
    }
}
