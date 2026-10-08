import { extensionChecks } from '../checks';

suite('Extension Test Suite', () => {
    for (const check of extensionChecks) {
        test(check.name, check.run);
    }
});
