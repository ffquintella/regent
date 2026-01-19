"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
const assert = __importStar(require("assert"));
const vscode = __importStar(require("vscode"));
suite('Extension Test Suite', () => {
    vscode.window.showInformationMessage('Start all tests.');
    test('Extension should be present', () => {
        assert.ok(vscode.extensions.getExtension('regent.regent'));
    });
    test('Should activate extension', async () => {
        const extension = vscode.extensions.getExtension('regent.regent');
        assert.ok(extension);
        if (!extension.isActive) {
            await extension.activate();
        }
        assert.strictEqual(extension.isActive, true);
    });
    test('Should register all commands', async () => {
        const extension = vscode.extensions.getExtension('regent.regent');
        if (!extension.isActive) {
            await extension.activate();
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
        assert.strictEqual(config.get('binaryPath'), 'regent');
        assert.strictEqual(config.get('lintOnSave'), false);
        assert.strictEqual(config.get('failOnWarnings'), false);
        assert.strictEqual(config.get('enableDiagnostics'), true);
    });
    test('Should handle missing workspace gracefully', async () => {
        // This test verifies error handling when no workspace is open
        // The command should show an error message and not throw
        try {
            await vscode.commands.executeCommand('regent.build');
            // If workspace is open, this passes
            assert.ok(true);
        }
        catch (err) {
            // Should not throw, should handle gracefully
            assert.fail('Command should handle missing workspace gracefully');
        }
    });
});
//# sourceMappingURL=extension.test.js.map