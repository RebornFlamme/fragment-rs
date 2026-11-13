// Banc du relevé DOM des glyphes (fr-p3.4) : une page de test dans l'Electron de Fragment, sans Fragment.
// Lancer depuis la racine du dépôt :
//   ../Fragment/app/node_modules/.bin/playwright test -c pixels/outils/glyphes
import { defineConfig } from '../../../../Fragment/app/node_modules/@playwright/test';

export default defineConfig({
    testDir: '.',
    outputDir: '../../.out/glyphes-banc',
    workers: 1,
    retries: 0,
    reporter: 'list',
    use: {
    },
});
