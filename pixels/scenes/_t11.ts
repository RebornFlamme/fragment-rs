// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T11 (thèmes de coffre, accent, réglages), côté Fragment (référence pixel).
//
//  Le coffre pixels/vault-t11 (fixture à nous : les notes de vault-t2 plus un thème « Rose » dans
//  `.fragment/themes/Rose/`) est copié dans un dossier au chemin FIXE « vault-t11 » : son nom
//  s'affiche au pied du dock gauche. Même décor que T2 (explorateur ouvert, pile centrale vide).
//
//  ★ Le thème « Rose » est écrit DEUX fois, avec les MÊMES jetons : `theme.css` (ce que lit
//    Fragment) et `theme.json` (ce que lit le candidat, ADR-005 §4). Un objet plat n'a pas de
//    variante sombre/claire : la palette est donc la même dans les deux schémas.
//  ★ Choisir un thème ou un accent = les réglages `cssTheme` et `accentColor` de l'apparence, comme
//    le font les réglages et `fragment theme:set`. Côté candidat : les commandes `pixels:theme-set`
//    (`args: {name}`) et `pixels:accent-set` (`args: {couleur}`, null = l'accent du thème).
//  ★ Le caret natif clignote (500 ms) : la scène d'éditeur le garde visible en capturant juste après
//    le clic, pendant la première demi-période allumée.
//
//  Fragment est piloté par son harnais e2e importé par chemin, jamais recopié. Ce fichier n'a aucun
//  test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, _electron, type ElectronApplication, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import path from 'path';
import { attendreLayoutReady } from '../../../Fragment/app/e2e/harness';
import { calme, schema, type Theme } from './_commun';
import { appliquerLayout, attendreFenetre, estApp, figer, layoutDecor, BASE_COFFRES, type ActionsT2, type Bouton } from './_t2';
import { activerModeInvisible } from '../invisible/garde';
activerModeInvisible(); // ★ fr-9x4 : tout lancement d'Electron passe par le mode invisible (refus sinon).

const RACINE = path.resolve(__dirname, '..');
const FIXTURE = path.join(RACINE, 'vault-t11');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

/** ★ Chemins FIXES, hors OneDrive et hors Fragment : le nom du coffre s'affiche à l'écran. */
export const COFFRE = `${BASE_COFFRES}/vault-t11`;
const DONNEES = `${BASE_COFFRES}/userdata`;

export interface ContexteT11 {
    app: ElectronApplication;
    p: Page;
    theme: Theme;
    act: ActionsT2;
    /** Écrit reference/<nom>-<thème>.png ; `caret: true` garde le caret natif (scène d'éditeur). */
    capturer(page?: Page, options?: { caret?: boolean }): Promise<void>;
    sonde(label: string, page: Page, selecteur: string, nth?: number): Promise<void>;
    /** `pixels:theme-set` : choisit le thème de coffre (`''` = celui de Fragment) et attend sa feuille. */
    choisirTheme(nom: string): Promise<void>;
    /** `pixels:accent-set` : choisit l'accent (`null` = celui du thème). */
    choisirAccent(couleur: string | null): Promise<void>;
}

/** Remet le coffre dans l'état de la fixture. */
function preparerCoffre(): void {
    rmSync(BASE_COFFRES, { recursive: true, force: true });
    mkdirSync(DONNEES, { recursive: true });
    cpSync(FIXTURE, COFFRE, { recursive: true });
    // Git ne garde pas un dossier vide : « Vide » porte un .gitkeep, que le coffre ne doit pas contenir.
    rmSync(path.join(COFFRE, 'Vide', '.gitkeep'), { force: true });
    if (!existsSync(path.join(COFFRE, '.fragment', 'themes', 'Rose', 'theme.css'))) throw new Error('thème Rose absent de la fixture');
}

async function lancer(): Promise<ElectronApplication> {
    preparerCoffre();
    const config = { vaultRoot: COFFRE, vaults: [{ path: COFFRE, lastOpened: 1_700_000_000_000 }] };
    writeFileSync(path.join(DONNEES, 'config.json'), JSON.stringify(config), 'utf8');
    return _electron.launch({
        args: ['.', `--user-data-dir=${DONNEES}`, '--force-device-scale-factor=1', '--high-dpi-support=1'],
        env: { ...process.env, NODE_ENV: 'development' } as Record<string, string>,
    });
}

async function dimensionner(app: ElectronApplication): Promise<void> {
    await app.evaluate(({ BrowserWindow }) => {
        for (const w of BrowserWindow.getAllWindows()) {
            w.webContents.closeDevTools();
            w.setContentSize(1280, 800);
        }
    });
}

function actionsDe(p: Page): ActionsT2 {
    return {
        command: (id) => p.evaluate((i) => (window as any).app.commands.executeCommandById(i), id).then(() => {}),
        click: (x, y, o: { button?: Bouton; count?: number; page?: Page } = {}) => (o.page ?? p).mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 }),
        hover: (x, y, page = p) => page.mouse.move(x, y),
        key: (t) => p.keyboard.press(t),
        type: (t) => p.keyboard.type(t),
        wait: (ms) => p.waitForTimeout(ms),
        waitStable: (page = p) => calme(page),
    };
}

function contexte(app: ElectronApplication, p: Page, nom: string, theme: Theme): ContexteT11 {
    return {
        app, p, theme, act: actionsDe(p),
        capturer: async (page = p, options = {}) => {
            const png = await page.screenshot({ animations: 'disabled', caret: options.caret ? 'initial' : 'hide' });
            writeFileSync(path.join(REFERENCE, `${nom}-${theme}.png`), png);
        },
        sonde: async (label, page, selecteur, n = 0) => {
            const b = await page.locator(selecteur).nth(n).boundingBox({ timeout: 2_000 }).catch(() => null);
            if (!b) return;
            mkdirSync(SONDES, { recursive: true });
            appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene: nom, theme, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2), box: b }) + '\n');
        },
        choisirTheme: async (nomTheme) => {
            await p.evaluate((n) => (window as any).app.appearance.set('cssTheme', n), nomTheme);
            // La feuille du thème est posée (ou retirée) de façon asynchrone.
            await p.waitForFunction((n) => {
                const f = document.getElementById('fragment-css-theme') as HTMLElement | null;
                return n === '' ? f === null : f !== null && f.dataset.theme === n;
            }, nomTheme, { timeout: 10_000 });
            await p.waitForTimeout(150);
        },
        choisirAccent: async (couleur) => {
            await p.evaluate((c) => (window as any).app.appearance.set('accentColor', c ?? ''), couleur);
            await p.waitForFunction((c) => (c === null
                ? !document.body.style.getPropertyValue('--color-accent')
                : document.body.style.getPropertyValue('--color-accent') !== ''), couleur, { timeout: 10_000 });
            await p.waitForTimeout(150);
        },
    };
}

/** Ouvre la fenêtre de réglages sur l'onglet `onglet` : 1000 × 700, rendu figé, pointeur garé. */
export async function ouvrirReglages(c: ContexteT11, onglet: string): Promise<Page> {
    const [r] = await Promise.all([
        c.app.waitForEvent('window', { timeout: 20_000 }),
        c.p.evaluate(() => (window as any).app.setting.open()),
    ]);
    await r.waitForFunction(() => !!document.querySelector('.vertical-tab-nav-item'), undefined, { timeout: 20_000 });
    await r.setViewportSize({ width: 1000, height: 700 }).catch(() => {});
    await figer(r);
    const nav = r.locator('.vertical-tab-nav-item', { hasText: onglet }).first();
    expect(await nav.count(), `onglet « ${onglet} » absent`).toBeGreaterThan(0);
    await c.sonde(`onglet ${onglet}`, r, `.vertical-tab-nav-item:has-text("${onglet}")`);
    await nav.click();
    await calme(r);
    return r;
}

export interface OptionsScene {
    /** Les schémas à capturer (défaut : sombre ET clair). */
    themes?: Theme[];
}

/**
 * Une scène de T11 : un test par schéma, l'app relancée à CHAQUE schéma. `corps` écrit
 * reference/<nom>-<schéma>.png par `capturer`. Décor de T2 : explorateur ouvert, pile centrale vide.
 */
export function sceneT11(nom: string, corps: (c: ContexteT11) => Promise<void>, options: OptionsScene = {}): void {
    const { themes = ['dark', 'light'] } = options;
    test.describe.serial(`pixels T11 — ${nom}`, () => {
        test.setTimeout(240_000);
        for (const theme of themes) {
            test(`${nom} — ${theme}`, async () => {
                mkdirSync(REFERENCE, { recursive: true });
                const app = await lancer();
                try {
                    const p = await attendreFenetre(app, estApp);
                    await attendreLayoutReady(p);
                    await dimensionner(app);
                    await figer(p);
                    await appliquerLayout(p, layoutDecor());
                    await schema(p, theme);
                    await p.bringToFront();
                    await calme(p);
                    await corps(contexte(app, p, nom, theme));
                } finally {
                    await app.close();
                }
            });
        }
    });
}

export { expect };
