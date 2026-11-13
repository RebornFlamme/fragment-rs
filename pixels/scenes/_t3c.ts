// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T3, groupe C (HTML dans le Markdown, menus de l'éditeur),
//  côté Fragment (référence pixel).
//
//  Le coffre pixels/vault-t3/c (fixture à nous, notes réécrites) est monté par le harnais du
//  lecteur. Le layout est ensuite REMPLACÉ : une seule pile au centre, portant UNE note ouverte,
//  docks vides et repliés (rien d'autre que la feuille de la note), caret masqué, rendu figé.
//  Les noms d'actions sont ceux du JSON de fragment-pixels (voir _t1.ts).
//
//  ★ Le curseur se pose par un clic aux MÊMES coordonnées des deux côtés ; le focus sans clic est
//  la commande `pixels:editor-focus` du JSON (ici : `act.focusEditeur()`). Un SVG animé est figé à
//  t = 0 (`figerSvg`, côté Fragment seulement) : le candidat dessine directement cette image fixe.
//
//  Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007). Ce fichier n'a aucun
//  test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { avecGlyphes } from './_glyphes';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { calme, schema, type Theme } from './_commun';
import { activerModeInvisible } from '../invisible/garde';
activerModeInvisible(); // ★ fr-9x4 : tout lancement d'Electron passe par le mode invisible (refus sinon).

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t3', 'c');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

export type Bouton = 'left' | 'right' | 'middle';

export interface ActionsT3c {
    /** `click {x,y,button,count}`. */
    click(x: number, y: number, options?: { button?: Bouton; count?: number }): Promise<void>;
    hover(x: number, y: number): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    /** `wait-stable` : polices prêtes, 2 images, 150 ms, SANS bouger le pointeur. */
    waitStable(): Promise<void>;
    /** `command pixels:editor-focus` : le focus dans l'éditeur, sans clic. */
    focusEditeur(): Promise<void>;
    /** Côté Fragment seulement : pause des animations SMIL des SVG et retour à t = 0 (image attendue du candidat). */
    figerSvg(): Promise<void>;
}

export interface ContexteT3c {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT3c;
    capturer(): Promise<void>;
    /** Note le centre d'un élément dans .out/coords.jsonl (pour écrire le JSON candidat). */
    sonde(label: string, selecteur: string, nth?: number): Promise<void>;
}

/** Le coffre pixels/vault-t3/c en mémoire : { 'x.md': Buffer }. */
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

/** Caret masqué, animations et transitions coupées (en plus de reducedMotion). */
async function figer(page: Page): Promise<void> {
    await page.emulateMedia({ reducedMotion: 'reduce' }).catch(() => {});
    await page.addStyleTag({
        content: '*,*::before,*::after{caret-color:transparent!important;animation:none!important;transition:none!important;scroll-behavior:auto!important}',
    });
}

const vide = () => ({ type: 'split', direction: 'vertical', children: [], collapsed: true });

/** Le layout du décor : une pile centrale, UNE feuille portant `fichier`, docks vides repliés. */
function layoutNote(fichier: string): unknown {
    const feuille = { type: 'leaf', id: 'pixels-t3c-note', state: { type: 'markdown', state: { file: fichier } } };
    return {
        version: 1,
        main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', currentTab: 0, children: [feuille] }] },
        left: vide(),
        right: vide(),
        active: 'pixels-t3c-note',
    };
}

/** Ouvre `fichier` dans une feuille neuve (l'éditeur est monté, l'état d'une passe précédente a disparu). */
async function ouvrirNote(p: Page, fichier: string): Promise<void> {
    await p.evaluate((l) => (window as any).app.workspace.changeLayout(l), layoutNote(fichier) as any);
    await p.waitForSelector('.doc-editor > .cm-editor .cm-content', { timeout: 20_000 });
    await p.waitForTimeout(300);
}

/**
 * Déclare une scène de T3 groupe C : un coffre monté une fois, puis un test par thème (sombre, clair)
 * qui rouvre `fichier` dans un layout neuf, joue `corps` et écrit reference/<nom>-<thème>.png.
 */
export function sceneT3c(nom: string, fichier: string, corps: (c: ContexteT3c) => Promise<void>): void {
    test.describe.serial(`pixels T3c — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await lancerLecteur(lireCoffre(), 'Autre note.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
            await h.electronApp.evaluate(({ BrowserWindow }) => {
                for (const w of BrowserWindow.getAllWindows()) w.setContentSize(1280, 800);
            });
            await figer(h.page);
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                const p = h.page;
                await ouvrirNote(p, fichier);
                await schema(p, theme);
                await calme(p);
                const act: ActionsT3c = {
                    click: (x, y, o = {}) => p.mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 }),
                    hover: (x, y) => p.mouse.move(x, y),
                    key: (t) => p.keyboard.press(t),
                    type: (t) => p.keyboard.type(t),
                    wait: (ms) => p.waitForTimeout(ms),
                    waitStable: () => calme(p),
                    focusEditeur: () => p.evaluate(() => (window as any).app.workspace.activeLeaf.view.editor.cm.focus()),
                    figerSvg: () => p.evaluate(() => {
                        for (const svg of document.querySelectorAll('svg')) {
                            const s = svg as unknown as SVGSVGElement;
                            if (typeof s.pauseAnimations === 'function') { s.pauseAnimations(); s.setCurrentTime(0); }
                        }
                    }),
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
            });
        }
    });
}
