import * as vscode from 'vscode';
import * as path from 'path';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

let diagnosticCollection: vscode.DiagnosticCollection;
let outputChannel: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('Regent');
    diagnosticCollection = vscode.languages.createDiagnosticCollection('regent');
    context.subscriptions.push(diagnosticCollection);
    context.subscriptions.push(outputChannel);

    // Register commands
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

export function deactivate() {
    if (diagnosticCollection) {
        diagnosticCollection.dispose();
    }
    if (outputChannel) {
        outputChannel.dispose();
    }
}

async function getRegentBinary(): Promise<string> {
    const config = vscode.workspace.getConfiguration('regent');
    return config.get<string>('binaryPath') || 'regent';
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
        vscode.window.showErrorMessage('No workspace folder open');
        return;
    }

    const regent = await getRegentBinary();
    const fullCommand = `${regent} ${command} ${args.join(' ')}`;

    outputChannel.show(true);
    outputChannel.appendLine(`\n$ ${fullCommand}`);
    outputChannel.appendLine('─'.repeat(80));

    try {
        const { stdout, stderr } = await execAsync(fullCommand, {
            cwd: workspaceRoot,
            maxBuffer: 10 * 1024 * 1024 // 10MB buffer
        });

        if (stdout) {
            outputChannel.appendLine(stdout);
        }
        if (stderr) {
            outputChannel.appendLine(stderr);
        }

        vscode.window.showInformationMessage(`Regent ${command} completed successfully`);
    } catch (error: any) {
        outputChannel.appendLine(`Error: ${error.message}`);
        if (error.stdout) {
            outputChannel.appendLine(error.stdout);
        }
        if (error.stderr) {
            outputChannel.appendLine(error.stderr);
        }
        vscode.window.showErrorMessage(`Regent ${command} failed: ${error.message}`);
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

        if (showOutput) {
            vscode.window.showInformationMessage('Regent lint completed');
        }
    } catch (error: any) {
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
    }
}

function parseLintResults(lintReport: any, workspaceRoot: string) {
    diagnosticCollection.clear();

    const diagnosticsMap = new Map<string, vscode.Diagnostic[]>();

    if (lintReport.results && Array.isArray(lintReport.results)) {
        for (const result of lintReport.results) {
            if (result.issues && Array.isArray(result.issues)) {
                for (const issue of result.issues) {
                    const filePath = path.join(workspaceRoot, issue.file || '');
                    const uri = vscode.Uri.file(filePath);
                    
                    const line = Math.max(0, (issue.line || 1) - 1);
                    const column = Math.max(0, (issue.column || 1) - 1);
                    const range = new vscode.Range(line, column, line, column + 100);

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

    // Apply all diagnostics
    for (const [filePath, diagnostics] of diagnosticsMap) {
        diagnosticCollection.set(vscode.Uri.file(filePath), diagnostics);
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
