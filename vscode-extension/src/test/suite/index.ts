import * as path from 'path';
import { glob } from 'glob';

export async function run(): Promise<void> {
    // Mocha 12 is ESM; load it without requiring the module from CommonJS.
    const { default: mochaConstructor } = await import('mocha');
    // Create the mocha test
    const mocha = new mochaConstructor({
        ui: 'tdd',
        color: true
    });

    const testsRoot = path.resolve(__dirname, '..');

    return new Promise((resolve, reject) => {
        glob('**/*.test.js', { cwd: testsRoot }).then((files) => {
            // Add files to the test suite
            files.forEach((file) => mocha.addFile(path.resolve(testsRoot, file)));

            try {
                // Run the mocha test
                mocha.run((failures: number) => {
                    if (failures > 0) {
                        reject(new Error(`${failures} tests failed.`));
                    } else {
                        resolve();
                    }
                });
            } catch (err) {
                console.error(err);
                reject(err);
            }
        }).catch(reject);
    });
}
