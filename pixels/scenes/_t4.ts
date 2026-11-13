// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T4 (PDF), côté Fragment (référence pixel).
//
//  Un PDF OUVERT SEUL dans une feuille : le coffre pixels/vault-t4 (nos PDF générés par
//  pixels/outils/pdf-fixture.mjs), un layout réduit à une pile d'un onglet (docks repliés, comme
//  _t1.ts), puis le fichier ouvert dans cette feuille. Rendu figé (caret, animations).
//
//  ★ Chaque geste de la scène passe par `act`, qui l'EXÉCUTE sur Fragment et l'ENREGISTRE sous la
//    forme de l'action JSON équivalente (schéma de pixels/README.md) : le JSON du candidat est écrit
//    par le même code que la référence (.out/actions/<scène>.json, lu au thème sombre), jamais
//    réestimé à la main.
//  ★ Zoom et page : la référence passe par l'interface (menu des modes) ou la vue, le JSON dit la
//    commande `pixels:pdf-zoom` / `pixels:pdf-page` (contrat de T4.md §5). L'état final est le même.
//
//  Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007). Pas de test ici.
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { avecGlyphes } from './_glyphes';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { calme, schema, type Theme } from './_commun';
import { layoutVide, type ArbreScene } from './_t1';
import { activerModeInvisible } from '../invisible/garde';
activerModeInvisible(); // ★ fr-9x4 : tout lancement d'Electron passe par le mode invisible (refus sinon).

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t4');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');
const ACTIONS = path.join(SONDES, 'actions');

/** Une seule feuille, un seul onglet : le PDF est seul. */
const UNE_FEUILLE: ArbreScene = { main: { type: 'tabs', tabs: 1, current: 0 } };

export interface Point { x: number; y: number }
export type Bouton = 'left' | 'right' | 'middle';
type Action = Record<string, unknown>;

export interface OptionsT4 {
    /** Fenêtre de la scène (défaut 1280 × 800). */
    fenetre?: { largeur: number; hauteur: number };
    /** Le PDF ouvert (défaut cours.pdf). */
    fichier?: string;
}

export interface ActionsT4 {
    click(x: number, y: number, options?: { button?: Bouton; count?: number }): Promise<void>;
    /** Clique au centre de l'élément (arrondi au pixel, même point dans le JSON). */
    cliquer(selecteur: string, nth?: number, options?: { count?: number }): Promise<void>;
    /** Clique l'entrée de menu dont le titre est exactement `titre`. */
    entreeMenu(titre: string): Promise<void>;
    hover(x: number, y: number): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    waitStable(): Promise<void>;
    drag(from: Point, to: Point, options?: { steps?: number }): Promise<void>;
    /** `pixels:pdf-zoom` : un mode (`auto`, `page-actual`, `page-fit`, `page-width`) ou une échelle. */
    zoom(arg: { mode: string } | { valeur: number }): Promise<void>;
    /** `pixels:pdf-page` : va au haut de la page `page`. */
    page(page: number): Promise<void>;
}

export interface ContexteT4 {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT4;
    capturer(): Promise<void>;
    /** Note le centre d'un élément dans .out/coords.jsonl (diagnostic). */
    sonde(label: string, selecteur: string, nth?: number): Promise<void>;
}

function lireCoffre(dir = COFFRE, prefixe = ''): Record<string, Buffer> {
    const out: Record<string, Buffer> = {};
    for (const nom of readdirSync(dir)) {
        const abs = path.join(dir, nom);
        const rel = prefixe ? `${prefixe}/${nom}` : nom;
        if (statSync(abs).isDirectory()) Object.assign(out, lireCoffre(abs, rel));
        else out[rel] = readFileSync(abs);
    }
    return out;
}

async function figer(page: Page): Promise<void> {
    await page.emulateMedia({ reducedMotion: 'reduce' }).catch(() => {});
    await page.addStyleTag({
        content: '*,*::before,*::after{caret-color:transparent!important;animation:none!important;transition:none!important;scroll-behavior:auto!important}',
    });
}

async function monter(fenetre: { largeur: number; hauteur: number }): Promise<Harnais> {
    const h = await lancerLecteur(lireCoffre(), 'note.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
    await h.electronApp.evaluate(({ BrowserWindow }, f) => {
        for (const w of BrowserWindow.getAllWindows()) w.setContentSize(f.largeur, f.hauteur);
    }, fenetre);
    await figer(h.page);
    return h;
}

/** Les pages du PDF sont peintes, le zoom est posé, la couche de texte est là. */
export async function pdfPret(p: Page): Promise<void> {
    await p.waitForSelector('.pdf-page canvas', { timeout: 20_000 });
    await expect(p.locator('.pdf-scroll.is-zooming')).toHaveCount(0);
    await expect(p.locator('.pdf-page canvas.is-stale')).toHaveCount(0);
    await expect(p.locator('.pdf-page .pdf-text-layer span').first()).toBeAttached({ timeout: 15_000 });
    await p.waitForTimeout(400);
}

/** Layout d'une feuille, PDF ouvert seul, barre latérale fermée, page 1. */
async function ouvrir(p: Page, fichier: string): Promise<void> {
    await p.evaluate((layout) => (window as any).app.workspace.changeLayout(layout), layoutVide(UNE_FEUILLE) as any);
    await p.waitForTimeout(200);
    await p.evaluate(async (f) => {
        const app = (window as any).app;
        const feuille = app.workspace.getLeavesOfType('empty')[0];
        await feuille.openFile(app.vault.getFileByPath(f), { active: true });
    }, fichier);
    await pdfPret(p);
    await expect(p.locator('.pdf-page-input')).toHaveValue('1');
    await expect(p.locator('.pdf-sidebar-container.is-open')).toHaveCount(0);
}

export function sceneT4(nom: string, corps: (c: ContexteT4) => Promise<void>, options: OptionsT4 = {}): void {
    const fenetre = options.fenetre ?? { largeur: 1280, hauteur: 800 };
    const fichier = options.fichier ?? 'cours.pdf';
    test.describe.serial(`pixels T4 — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await monter(fenetre);
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                const p = h.page;
                await schema(p, theme);
                await ouvrir(p, fichier);
                await calme(p);
                const log: Action[] = [];
                const noter = (a: Action) => { log.push(a); };
                const centre = async (sel: string, nth = 0): Promise<Point> => {
                    const b = await p.locator(sel).nth(nth).boundingBox();
                    if (!b) throw new Error(`élément introuvable : ${sel}[${nth}]`);
                    return { x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2) };
                };
                const act: ActionsT4 = {
                    click: async (x, y, o = {}) => {
                        await p.mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 });
                        noter({ type: 'click', x, y, ...(o.button && o.button !== 'left' ? { button: o.button } : {}), ...(o.count && o.count > 1 ? { count: o.count } : {}) });
                    },
                    cliquer: async (sel, nth = 0, o = {}) => {
                        const { x, y } = await centre(sel, nth);
                        await act.click(x, y, o);
                    },
                    entreeMenu: async (titre) => {
                        const titres = await p.locator('.menu .menu-item-title').allTextContents();
                        const n = titres.findIndex((t) => t === titre);
                        if (n < 0) throw new Error(`entrée de menu introuvable : ${titre} (${titres.join(' | ')})`);
                        await act.cliquer('.menu .menu-item', n);
                    },
                    hover: async (x, y) => { await p.mouse.move(x, y); noter({ type: 'hover', x, y }); },
                    key: async (k) => { await p.keyboard.press(k); noter({ type: 'key', keys: k }); },
                    type: async (t) => { await p.keyboard.type(t); noter({ type: 'type', text: t }); },
                    wait: async (ms) => { await p.waitForTimeout(ms); noter({ type: 'wait', ms }); },
                    waitStable: async () => { await calme(p); noter({ type: 'wait-stable' }); },
                    drag: async (from, to, o = {}) => {
                        await p.mouse.move(from.x, from.y);
                        await p.mouse.down();
                        await p.mouse.move(to.x, to.y, { steps: o.steps ?? 10 });
                        await p.mouse.up();
                        noter({ type: 'drag', from, to });
                    },
                    zoom: async (arg) => {
                        // La référence passe par le menu ▾ du zoom ; le texte de l'entrée suit le mode.
                        const titres: Record<string, string> = { auto: 'Automatic', 'page-actual': 'Actual size', 'page-fit': 'Page fit', 'page-width': 'Page width' };
                        const titre = 'mode' in arg ? titres[arg.mode] : `${Math.round(arg.valeur * 100)}%`;
                        const bouton = await centre('.pdf-toolbar [aria-label="Zoom options"]');
                        await p.mouse.click(bouton.x, bouton.y);
                        const titresMenu = await p.locator('.menu .menu-item-title').allTextContents();
                        const n = titresMenu.findIndex((t) => t === titre);
                        if (n < 0) throw new Error(`mode de zoom introuvable : ${titre} (${titresMenu.join(' | ')})`);
                        await p.locator('.menu .menu-item').nth(n).click();
                        await pdfPret(p);
                        await calme(p);
                        noter({ type: 'command', id: 'pixels:pdf-zoom', args: arg });
                    },
                    page: async (n) => {
                        await p.evaluate((page) => (window as any).app.workspace.activeLeaf.view.allerALaPage(page), n);
                        await expect(p.locator('.pdf-page-input')).toHaveValue(String(n));
                        await pdfPret(p);
                        noter({ type: 'command', id: 'pixels:pdf-page', args: { page: n } });
                    },
                };
                await corps({
                    h, p, theme, act,
                    capturer: async () => {
                        const fichier = path.join(REFERENCE, `${nom}-${theme}.png`);
                        const png = await avecGlyphes(p, fichier, () => p.screenshot({ animations: 'disabled', caret: 'hide' }));
                        writeFileSync(fichier, png);
                    },
                    sonde: async (label, selecteur, n = 0) => {
                        const b = await p.locator(selecteur).nth(n).boundingBox();
                        if (!b) return;
                        mkdirSync(SONDES, { recursive: true });
                        appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene: nom, theme, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2), box: b }) + '\n');
                    },
                });
                // Le JSON du candidat : le PDF ouvert seul, puis les gestes joués, puis le pointeur garé.
                if (theme === 'dark') {
                    const scene = {
                        scene: nom,
                        depuis: 'T4',
                        window: { width: fenetre.largeur, height: fenetre.hauteur, dpr: 1 },
                        themes: ['dark', 'light'],
                        vault: 'pixels/vault-t4',
                        actions: [
                            { type: 'layout', tree: { main: { type: 'tabs', tabs: 1, current: 0 } } },
                            { type: 'open', path: fichier },
                            { type: 'wait-stable' },
                            ...log,
                            { type: 'hover', x: 2, y: 2 },
                            { type: 'wait-stable' },
                        ],
                    };
                    mkdirSync(ACTIONS, { recursive: true });
                    const lignes = scene.actions.map((a) => '    ' + JSON.stringify(a));
                    const entete = JSON.stringify({ ...scene, actions: undefined }, null, 2).replace(/\n}$/, '');
                    writeFileSync(path.join(ACTIONS, `${nom}.json`), `${entete.replace(/"themes": \[\s+"dark",\s+"light"\s+\]/, '"themes": ["dark","light"]').replace(/"window": \{[^}]*\}/, `"window": {"width":${fenetre.largeur},"height":${fenetre.hauteur},"dpr":1}`)},\n  "actions": [\n${lignes.join(',\n')}\n  ]\n}\n`);
                }
            });
        }
    });
}

export { expect };
