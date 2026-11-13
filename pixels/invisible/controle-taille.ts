// Scènes de CONTRÔLE du mode invisible (fr-278) : la fenêtre sans cadre de Fragment, mise à une taille
// de contenu donnée hors moniteur, doit sortir EXACTEMENT à cette taille (zone cliente et PNG).
//
// ★ t3-etroite (600 × 400, la taille minimale de Fragment) sortait en 616 × 408 : hors moniteur, la zone
//   cliente d'une fenêtre sans cadre dépasse ses bornes de 16 × 8 px, et le minimum de Fragment, qui
//   borne les BORNES, refusait la correction. Ces deux contrôles (au minimum exact, et 1 px au-dessus)
//   prouvent la compensation de crochet.cjs (compenserMinimum).
//
// Preuves seulement : rien n'est écrit dans pixels/reference (pas de PNG de référence, pas de JSON de
// scène côté candidat). Le PNG va dans pixels/.out/controle-taille/ pour inspection.
// Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007).
import { expect, test } from '../../../Fragment/app/node_modules/@playwright/test';
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { activerModeInvisible } from './garde';
activerModeInvisible();

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t3', 'b');
const SORTIE = path.join(RACINE, '.out', 'controle-taille');

/** Largeur × hauteur lues dans l'en-tête IHDR d'un PNG. */
function taillePng(png: Buffer): [number, number] {
    return [png.readUInt32BE(16), png.readUInt32BE(20)];
}

export function controleTaille(nom: string, largeur: number, hauteur: number): void {
    test.describe.serial(`contrôle mode invisible — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            const notes: Record<string, Buffer> = {};
            for (const f of readdirSync(COFFRE)) if (f.endsWith('.md')) notes[f] = readFileSync(path.join(COFFRE, f));
            h = await lancerLecteur(notes, 'etroite.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        test(`${nom} — ${largeur} × ${hauteur}`, async () => {
            await h.electronApp.evaluate(({ BrowserWindow }, [l, ht]) => {
                for (const f of BrowserWindow.getAllWindows()) f.setContentSize(l, ht);
            }, [largeur, hauteur]);
            await h.page.waitForTimeout(300);
            const contenus = await h.electronApp.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((f) => f.getContentSize()));
            const png = await h.page.screenshot({ animations: 'disabled', caret: 'hide' });
            mkdirSync(SORTIE, { recursive: true });
            writeFileSync(path.join(SORTIE, `${nom}.png`), png);
            console.log(`[contrôle] ${nom} : contenu ${JSON.stringify(contenus)}, PNG ${taillePng(png).join('×')}`);
            for (const c of contenus) expect(c).toEqual([largeur, hauteur]);
            expect(taillePng(png)).toEqual([largeur, hauteur]);
        });
    });
}
