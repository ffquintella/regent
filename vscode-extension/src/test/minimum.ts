import { extensionChecks } from './checks';

// Run the same assertions on the minimum VS Code host without loading Mocha.
export async function run(): Promise<void> {
    let failures = 0;
    for (const check of extensionChecks) {
        try {
            await check.run();
            console.log(`PASS: ${check.name}`);
        } catch (error) {
            failures++;
            console.error(`FAIL: ${check.name}`, error);
        }
    }
    console.log(`${extensionChecks.length - failures} passing, ${failures} failing`);
    if (failures > 0) {
        throw new Error(`${failures} extension checks failed.`);
    }
}
