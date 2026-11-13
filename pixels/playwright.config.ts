// ★ Le Playwright de Fragment/app/node_modules (jamais un second exemplaire :
//   le harnais e2e importé de Fragment résout le même paquet).
import { defineConfig } from '../../Fragment/app/node_modules/@playwright/test';
import { activerModeInvisible, modeInvisibleActif } from './invisible/garde';

// ★★ MODE INVISIBLE SEUL (fr-9x4, décision de l'utilisateur du 13/11) : Electron ne vient JAMAIS
//   devant l'utilisateur. La config est chargée par le lanceur ET par chaque processus de test : les
//   deux enveloppent `_electron.launch` (crochet hors écran + sentinelle de preuves), ou refusent de
//   démarrer. Il n'existe pas d'autre mode.
activerModeInvisible();
if (!modeInvisibleActif()) throw new Error('mode invisible non actif : démarrage refusé.');

/**
 * Config des scènes de référence pixel (Fragment pilotée par son harnais e2e).
 * Lancer par `scripts/pixels-reference.ps1` (le cwd DOIT être Fragment/app : le
 * harnais fait `electron .` et lit `e2e/fixtures/...` relativement).
 */
export default defineConfig({
    testDir: './scenes',
    // Sorties de Playwright DANS fragment-rs (jamais dans Fragment).
    outputDir: './.out',
    fullyParallel: false,
    workers: 1,
    retries: 0,
    timeout: 240_000,
    // Pas de rapport html : il s'écrirait dans le cwd (Fragment/app).
    reporter: 'list',
    use: {
        viewport: { width: 1280, height: 800 },
        deviceScaleFactor: 1,
        reducedMotion: 'reduce',
        colorScheme: 'dark',
    },
    // ★ Pas de `webServer` : le harnais charge la fenêtre depuis le port 5123 (codé en dur dans
    // Fragment). Ce serveur est la Vite enveloppe (pixels/vite.reference.config.ts), démarrée —
    // ou reconnue si l'utilisateur en a déjà une — par scripts/pixels-reference.ps1. Une commande
    // `npm run dev:react` lancée d'ici écrirait le cache Vite dans Fragment.
});
