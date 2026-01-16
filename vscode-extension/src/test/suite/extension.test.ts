import * as assert from 'assert';
import * as vscode from 'vscode';

suite('Extension Test Suite', () => {
    vscode.window.showInformationMessage('Start all tests.');

    test('Extension should be present', () => {
        assert.ok(vscode.extensions.getExtension('regent.regent'));
    });

    test('Should activate extension', async () => {
        const extension = vscode.extensions.getExtension('regent.regent');
        assert.ok(extension);
        
        if (!extension!.isActive) {
            await extension!.activate();
        }
        
        assert.strictEqual(extension!.isActive, true);
    });

    test('Should register all commands', async () => {
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
    });

    test('Configuration should have correct defaults', () => {
        const config = vscode.workspace.getConfiguration('regent');
        
        assert.strictEqual(config.get<string>('binaryPath'), 'regent');
        assert.strictEqual(config.get<boolean>('lintOnSave'), false);
        assert.strictEqual(config.get<boolean>('failOnWarnings'), false);
        assert.strictEqual(config.get<boolean>('enableDiagnostics'), true);
    });

    test('Should handle missing workspace gracefully', async () => {
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
    });
});
