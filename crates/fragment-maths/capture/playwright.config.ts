// Config de la capture de référence du spike S5 (maths) : pilote Fragment par son harnais e2e,
// sans rien écrire dans son arbre (sorties ici, dans .out/ et ../reference/).
import { defineConfig } from '../../../../Fragment/app/node_modules/@playwright/test';

export default defineConfig({
    testDir: '.',
    testMatch: /maths\.spec\.ts$/,
    outputDir: './.out',
    workers: 1,
    retries: 0,
    timeout: 240_000,
    reporter: 'list',
    use: { deviceScaleFactor: 1, reducedMotion: 'reduce', colorScheme: 'dark' },
});
