import * as path from 'path';
import { runTests } from '@vscode/test-electron';

async function main() {
    try {
        // The folder containing the Extension Manifest package.json
        const extensionDevelopmentPath = path.resolve(__dirname, '../../');

        // The path to test runner
        const minimum = process.argv.includes('--minimum');
        const extensionTestsPath = path.resolve(__dirname, minimum ? './minimum' : './suite/index');

        // Download VS Code, unzip it and run the integration test
        await runTests({ 
            version: process.env.VSCODE_TEST_VERSION || (minimum ? '1.85.2' : 'stable'),
            extensionDevelopmentPath, 
            extensionTestsPath,
            launchArgs: ['--disable-extensions']
        });
    } catch (err) {
        console.error('Failed to run tests', err);
        process.exit(1);
    }
}

main();
