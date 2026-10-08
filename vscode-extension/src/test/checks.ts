import * as assert from 'assert';
import * as vscode from 'vscode';

export async function extensionShouldBePresent(): Promise<void> {
    assert.ok(vscode.extensions.getExtension('regent.regent'));
}

export async function shouldActivateExtension(): Promise<void> {
    const extension = vscode.extensions.getExtension('regent.regent');
    assert.ok(extension);

    if (!extension!.isActive) {
        await extension!.activate();
    }

    assert.strictEqual(extension!.isActive, true);
}

export async function shouldRegisterAllCommands(): Promise<void> {
    const extension = vscode.extensions.getExtension('regent.regent');
    if (!extension!.isActive) {
        await extension!.activate();
    }

    const commands = await vscode.commands.getCommands(true);

    const regentCommands = [
        'regent.showMenu',
        'regent.build',
        'regent.test',
        'regent.lint',
        'regent.validators',
        'regent.generate',
        'regent.fixAll',
        'regent.setupWorkspace'
    ];

    regentCommands.forEach(cmd => {
        assert.ok(commands.includes(cmd), `Command ${cmd} should be registered`);
    });
}

export async function configurationShouldHaveCorrectDefaults(): Promise<void> {
    const config = vscode.workspace.getConfiguration('regent');

    assert.strictEqual(config.get<string>('binaryPath'), 'regent');
    assert.strictEqual(config.get<boolean>('lintOnSave'), false);
    assert.strictEqual(config.get<boolean>('failOnWarnings'), false);
    assert.strictEqual(config.get<boolean>('enableDiagnostics'), true);
}

export async function shouldHandleMissingWorkspaceGracefully(): Promise<void> {
    // This test verifies error handling when no workspace is open
    // The command should show an error message and not throw
    try {
        await vscode.commands.executeCommand('regent.build');
        // If workspace is open, this passes
        assert.ok(true);
    } catch (err) {
        // Should not throw, should handle gracefully
        assert.fail('Command should handle missing workspace gracefully');
    }
}

export const extensionChecks = [
    { name: 'Extension should be present', run: extensionShouldBePresent },
    { name: 'Should activate extension', run: shouldActivateExtension },
    { name: 'Should register all commands', run: shouldRegisterAllCommands },
    { name: 'Configuration should have correct defaults', run: configurationShouldHaveCorrectDefaults },
    { name: 'Should handle missing workspace gracefully', run: shouldHandleMissingWorkspaceGracefully }
];
