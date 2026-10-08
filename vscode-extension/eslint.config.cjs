const parser = require('@typescript-eslint/parser');
const plugin = require('@typescript-eslint/eslint-plugin');

module.exports = [
    { ignores: ['out/**', 'dist/**', '**/*.d.ts', '.vscode-test/**'] },
    {
        files: ['src/**/*.ts'],
        languageOptions: { parser, ecmaVersion: 'latest', sourceType: 'module' },
        plugins: { '@typescript-eslint': plugin },
        rules: {
            '@typescript-eslint/naming-convention': 'warn',
            curly: 'warn',
            eqeqeq: 'warn',
            'no-throw-literal': 'warn',
            semi: ['warn', 'always']
        }
    }
];
