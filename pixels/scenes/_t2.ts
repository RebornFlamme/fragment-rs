// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T2, côté Fragment (référence pixel).
//
//  Le coffre pixels/vault-t2 (fixture à nous) est copié dans un dossier au chemin FIXE, nommé
//  « vault-t2 » : le nom du dossier est celui que la barre du pied du dock gauche affiche, et le
//  chemin se lit dans le gestionnaire de coffres. Un mkdtemp (comme le fait le harnais) donnerait
//  un nom différent à chaque passe, et un chemin que le candidat ne peut pas reproduire.
//
//  Le décor monte l'explorateur ouvert au dock gauche et une pile centrale VIDE (aucune vue de
//  fichier avant T3), puis fige caret et animations. Les noms d'actions sont ceux du JSON de
//  fragment-pixels (voir _t1.ts) : une scène s'écrit une fois dans chaque langage.
//
//  Fragment est piloté par son harnais e2e importé par chemin (`fenetreApp`, `attendreLayoutReady`),
//  jamais recopié. Ce fichier n'a aucun test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, _electron, type ElectronApplication, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'fs';
import path from 'path';
import { attendreLayoutReady, fenetreApp } from '../../../Fragment/app/e2e/harness';
import { calme, schema, type Theme } from './_commun';
import { activerModeInvisible } from '../invisible/garde';
activerModeInvisible(); // ★ fr-9x4 : tout lancement d'Electron passe par le mode invisible (refus sinon).

const RACINE = path.resolve(__dirname, '..');
const FIXTURE = path.join(RACINE, 'vault-t2');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

/** ★ Chemins FIXES, hors OneDrive et hors Fragment : ils s'affichent dans le gestionnaire de coffres. */
export const BASE_COFFRES = 'C:/dev/fragment-rs-pixels-coffres';
export const COFFRE = `${BASE_COFFRES}/vault-t2`;
/** Un coffre connu dont le dossier n'existe pas (scène « gestionnaire-liste »). */
export const COFFRE_DISPARU = `${BASE_COFFRES}/disparu`;
const DONNEES = `${BASE_COFFRES}/userdata`;

export type Bouton = 'left' | 'right' | 'middle';
export interface Point { x: number; y: number }

export interface ActionsT2 {
    command(id: string): Promise<void>;
    click(x: number, y: number, options?: { button?: Bouton; count?: number; page?: Page }): Promise<void>;
    hover(x: number, y: number, page?: Page): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    /** `wait-stable` : polices prêtes, 2 images, 150 ms, SANS bouger le pointeur. */
    waitStable(page?: Page): Promise<void>;
}

export interface ContexteT2 {
    app: ElectronApplication;
    p: Page;
    theme: Theme;
    act: ActionsT2;
    capturer(page?: Page): Promise<void>;
    /** Note le centre d'un élément dans .out/coords.jsonl (pour écrire le JSON candidat). */
    sonde(label: string, page: Page, selecteur: string, nth?: number): Promise<void>;
}

/** Ce que le décor met dans le fichier de configuration du processus principal. */
export interface ConfigCoffres {
    /** Le coffre ouvert ; null/absent = premier lancement (le gestionnaire s'ouvre seul). */
    vaultRoot?: string | null;
    vaults?: { path: string; lastOpened: number }[];
}

/** Caret masqué, animations et transitions coupées (en plus de reducedMotion). */
export async function figer(page: Page): Promise<void> {
    await page.emulateMedia({ reducedMotion: 'reduce' }).catch(() => {});
    await page.addStyleTag({
        content: '*,*::before,*::after{caret-color:transparent!important;animation:none!important;transition:none!important;scroll-behavior:auto!important}',
    });
}

/** Remet le coffre dans l'état de la fixture (un renommage ou une création d'une passe précédente disparaît). */
export function preparerCoffre(): void {
    rmSync(BASE_COFFRES, { recursive: true, force: true });
    mkdirSync(DONNEES, { recursive: true });
    cpSync(FIXTURE, COFFRE, { recursive: true });
    // Git ne garde pas un dossier vide : « Vide » porte un .gitkeep, que le coffre ne doit pas contenir.
    rmSync(path.join(COFFRE, 'Vide', '.gitkeep'), { force: true });
    if (existsSync(COFFRE_DISPARU)) rmSync(COFFRE_DISPARU, { recursive: true, force: true });
}

/** Lance Fragment (Electron) sur la configuration donnée, 1280 × 800 à DPR 1. Rend l'app et ses fenêtres. */
export async function lancer(config: ConfigCoffres): Promise<ElectronApplication> {
    preparerCoffre();
    writeFileSync(path.join(DONNEES, 'config.json'), JSON.stringify(config), 'utf8');
    // ★ `--disable-lcd-text` : texte en niveaux de gris, comme les références de T1 et comme GPUI.
    // La capture de #80 s'est faite avec ClearType actif (chroma du texte jusqu'à 161, contre ≤ 5 en
    // T1) : chaque boîte de texte s'élargissait d'1 à 2 px. Le drapeau rend la capture indépendante
    // du réglage de Windows.
    return _electron.launch({
        args: ['.', `--user-data-dir=${DONNEES}`, '--force-device-scale-factor=1', '--high-dpi-support=1', '--disable-lcd-text'],
        env: { ...process.env, NODE_ENV: 'development' } as Record<string, string>,
    });
}

export const estGestion = (url: string): boolean => url.includes('#vault-manager');
export const estApp = (url: string): boolean => url.includes('localhost:5123') && !estGestion(url);

/** Attend une fenêtre dont l'URL satisfait le prédicat. */
export async function attendreFenetre(app: ElectronApplication, predicat: (url: string) => boolean, delai = 30_000): Promise<Page> {
    const fin = Date.now() + delai;
    while (Date.now() < fin) {
        for (const w of app.windows()) if (predicat(w.url())) return w;
        await new Promise((r) => setTimeout(r, 150));
    }
    throw new Error('fenêtre introuvable');
}

/** Fixe la taille de contenu de la fenêtre principale (1280 × 800). */
async function dimensionner(app: ElectronApplication): Promise<void> {
    await app.evaluate(({ BrowserWindow }) => {
        for (const w of BrowserWindow.getAllWindows()) {
            w.webContents.closeDevTools();
            w.setContentSize(1280, 800);
        }
    });
}

/** Le layout du décor : explorateur au dock gauche (ouvert), une pile centrale vide, dock droit replié. */
export function layoutDecor(): unknown {
    const feuille = (id: string, vue: string) => ({ type: 'leaf', id, state: { type: vue, state: {} } });
    return {
        version: 1,
        main: { type: 'split', direction: 'vertical', children: [
            { type: 'tabs', currentTab: 0, children: [feuille('pixels-t2-vide', 'empty')] },
        ] },
        left: { type: 'split', direction: 'horizontal', collapsed: false, children: [
            { type: 'tabs', currentTab: 0, children: [feuille('pixels-t2-explorateur', 'file-explorer')] },
        ] },
        right: { type: 'split', direction: 'horizontal', collapsed: true, children: [] },
        active: null,
    };
}

export async function appliquerLayout(p: Page, layout: unknown): Promise<void> {
    await p.evaluate((l) => (window as any).app.workspace.changeLayout(l), layout as any);
    await p.waitForTimeout(300);
}

/** Les actions de scène sur la fenêtre `p` (mêmes noms que le JSON). */
function actionsDe(p: Page): ActionsT2 {
    return {
        command: (id) => p.evaluate((i) => (window as any).app.commands.executeCommandById(i), id).then(() => {}),
        click: (x, y, o = {}) => (o.page ?? p).mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 }),
        hover: (x, y, page = p) => page.mouse.move(x, y),
        key: (t) => p.keyboard.press(t),
        type: (t) => p.keyboard.type(t),
        wait: (ms) => p.waitForTimeout(ms),
        waitStable: (page = p) => calme(page),
    };
}

function contexte(app: ElectronApplication, p: Page, nom: string, theme: Theme): ContexteT2 {
    return {
        app, p, theme, act: actionsDe(p),
        capturer: async (page = p) => {
            const png = await page.screenshot({ animations: 'disabled', caret: 'hide' });
            writeFileSync(path.join(REFERENCE, `${nom}-${theme}.png`), png);
        },
        sonde: async (label, page, selecteur, n = 0) => {
            const b = await page.locator(selecteur).nth(n).boundingBox({ timeout: 2_000 }).catch(() => null);
            if (!b) return;
            mkdirSync(SONDES, { recursive: true });
            appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene: nom, theme, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2), box: b }) + '\n');
        },
    };
}

/** Les réglages d'une scène de T2. */
export interface OptionsScene {
    /** Configuration du processus principal (défaut : le coffre vault-t2 ouvert). */
    config?: ConfigCoffres;
    /** `decor` (défaut) : explorateur au dock gauche + pile vide ; `defaut` : le layout par défaut de Fragment, tel quel. */
    layout?: 'decor' | 'defaut';
    /** La fenêtre pilotée : l'app (défaut) ou le gestionnaire de coffres (fenêtre à part, 840 × 560). */
    fenetre?: 'app' | 'gestion';
}

const CONFIG_OUVERT: ConfigCoffres = { vaultRoot: COFFRE, vaults: [{ path: COFFRE, lastOpened: 1_700_000_000_000 }] };

/**
 * Une scène de T2 : un test par thème (sombre, clair), l'app relancée à CHAQUE thème (les scènes de
 * renommage et de menus modifient le coffre). `corps` écrit reference/<nom>-<thème>.png par `capturer`.
 * Par défaut : coffre vault-t2 ouvert, explorateur au dock gauche, pile centrale vide, rendu figé.
 */
export function sceneT2(nom: string, corps: (c: ContexteT2) => Promise<void>, options: OptionsScene = {}): void {
    const { config = CONFIG_OUVERT, layout = 'decor', fenetre = 'app' } = options;
    test.describe.serial(`pixels T2 — ${nom}`, () => {
        test.setTimeout(240_000);
        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                mkdirSync(REFERENCE, { recursive: true });
                const app = await lancer(config);
                try {
                    if (fenetre === 'gestion') {
                        const g = await attendreFenetre(app, estGestion);
                        await g.waitForSelector('.vault-manager-title', { timeout: 20_000 });
                        // ★ La fenêtre du gestionnaire n'a pas de moteur d'apparence : le thème est posé à la main (la fenêtre suit sinon le réglage du système).
                        await g.evaluate((t) => {
                            // Les deux marques que pose appliquerSchema (core/settings/theme) : classe du <body> et attribut.
                            document.body.classList.toggle('theme-dark', t === 'dark');
                            document.body.classList.toggle('theme-light', t === 'light');
                            document.documentElement.dataset.theme = t;
                            document.documentElement.style.colorScheme = t;
                        }, theme);
                        await figer(g);
                        await g.bringToFront();
                        await calme(g);
                        await corps(contexte(app, g, nom, theme));
                    } else {
                        const p = await attendreFenetre(app, estApp);
                        await attendreLayoutReady(p);
                        await dimensionner(app);
                        await figer(p);
                        if (layout === 'decor') await appliquerLayout(p, layoutDecor());
                        await schema(p, theme);
                        await p.bringToFront();
                        await calme(p);
                        await corps(contexte(app, p, nom, theme));
                    }
                } finally {
                    await app.close();
                }
            });
        }
    });
}

export { expect };
