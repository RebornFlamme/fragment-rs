// ═══════════════════════════════════════════════════════════════════════════
//  Décor VIDE des scènes de T1, côté Fragment (référence pixel).
//
//  À la différence de _commun.ts (note + PDF + chat, scènes T3 et suivantes), ce décor monte le
//  coffre pixels/vault, REMPLACE le layout par des onglets de vues vides (docks repliés : le
//  layout par défaut de Fragment contient l'explorateur, qui est de T2), puis fige caret et
//  animations. Il expose les MÊMES noms d'actions que le JSON de fragment-pixels : une scène
//  s'écrit une fois dans chaque langage, action pour action.
//
//  ★ NON EXÉCUTÉ à la livraison de `fr-t1-lecteur` : le lanceur de références (config Vite
//  enveloppe) est écrit par `fr-t1-references` en parallèle. Écrit d'après le harnais, jamais
//  rejoué ; le premier passage réel peut demander des retouches de sélecteurs.
//
//  Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007). Ce fichier n'a aucun
//  test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { calme, schema, type Theme } from './_commun';

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

// ── L'arbre de scène (même forme que l'action JSON `layout`) ────────────────

/** Une pile d'onglets de vues vides : `tabs` onglets, le n°`current` (0-based) actif. */
export interface PileScene {
    type: 'tabs';
    tabs: number;
    current?: number;
    dimension?: number;
}
/** Un découpage : `vertical` = enfants côte à côte, `horizontal` = empilés. */
export interface DecoupeScene {
    type: 'split';
    direction: 'vertical' | 'horizontal';
    children: NoeudScene[];
    dimension?: number;
}
export type NoeudScene = PileScene | DecoupeScene;

/** Un dock latéral : replié par défaut dans les scènes de T1 (il n'a rien à montrer). */
export interface DockScene {
    collapsed?: boolean;
    width?: number;
}
export interface ArbreScene {
    main: NoeudScene;
    left?: DockScene;
    right?: DockScene;
}

let compteurFeuilles = 0;

/** Traduit un nœud de scène vers la forme sérialisée d'un layout de Fragment (version 1). */
function versNoeud(n: NoeudScene): unknown {
    if (n.type === 'split') {
        return { type: 'split', direction: n.direction, dimension: n.dimension, children: n.children.map(versNoeud) };
    }
    const feuilles = Array.from({ length: n.tabs }, () => ({
        type: 'leaf',
        id: `pixels-feuille-${++compteurFeuilles}`,
        state: { type: 'empty', state: {} },
    }));
    return { type: 'tabs', dimension: n.dimension, currentTab: n.current ?? 0, children: feuilles };
}

function versDock(d: DockScene = { collapsed: true }): unknown {
    return { type: 'split', direction: 'vertical', children: [], collapsed: d.collapsed ?? true, width: d.width };
}

/** Le layout complet : racine centrale d'après `arbre`, docks vides (repliés sauf avis contraire). */
export function layoutVide(arbre: ArbreScene): unknown {
    const main = versNoeud(arbre.main);
    return {
        version: 1,
        main: main && (main as any).type === 'split' ? main : { type: 'split', direction: 'vertical', children: [main] },
        left: versDock(arbre.left),
        right: versDock(arbre.right),
        active: null,
    };
}

/** Une pile de `n` onglets vides, le 2e actif : le décor de base de T1. */
export const PILE_PAR_DEFAUT: ArbreScene = { main: { type: 'tabs', tabs: 4, current: 1 } };

// ── Le contexte d'une scène : les mêmes noms d'actions que le JSON ──────────

export type Bouton = 'left' | 'right' | 'middle';
export interface Point { x: number; y: number }

export interface ActionsT1 {
    /** `command {id}` : exécute une commande du CommandManager de Fragment. */
    command(id: string): Promise<void>;
    /** `layout {tree}` : remplace l'arbre par des onglets de vues vides. */
    layout(arbre: ArbreScene): Promise<void>;
    /** `click {x,y,button,count}` ; `reglages` = clic dans la fenêtre de réglages. */
    click(x: number, y: number, options?: { button?: Bouton; count?: number; page?: Page }): Promise<void>;
    hover(x: number, y: number, page?: Page): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    /** `wait-stable` : polices prêtes, 2 images, 150 ms, SANS bouger le pointeur. */
    waitStable(page?: Page): Promise<void>;
    /**
     * `drag {from,to,release}` : un geste continu à la souris. `release: false` laisse le bouton
     * enfoncé — la capture suivante montre le glisser EN COURS (zone de dépôt), puis `relacher()`.
     */
    drag(from: Point, to: Point, options?: { release?: boolean; steps?: number; button?: Bouton }): Promise<void>;
    relacher(): Promise<void>;
}

export interface ContexteT1 {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT1;
    /** Capture `p` (ou une autre fenêtre) vers reference/<scène>-<thème>.png. */
    capturer(page?: Page): Promise<void>;
    /** Note le centre d'un élément dans .out/coords.jsonl (pour écrire le JSON candidat). */
    sonde(label: string, page: Page, selecteur: string, nth?: number): Promise<void>;
}

/** Le coffre pixels/vault/ en mémoire : { 'dossier/x.md': Buffer }. */
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

async function appliquerLayout(p: Page, arbre: ArbreScene): Promise<void> {
    await p.evaluate((layout) => (window as any).app.workspace.changeLayout(layout), layoutVide(arbre) as any);
    await p.waitForTimeout(200);
}

/** Le décor de T1 : coffre monté, fenêtre 1280 × 800, layout VIDE, rendu figé. */
async function monter(arbre: ArbreScene): Promise<Harnais> {
    const h = await lancerLecteur(lireCoffre(), 'note.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
    await h.electronApp.evaluate(({ BrowserWindow }) => {
        for (const w of BrowserWindow.getAllWindows()) w.setContentSize(1280, 800);
    });
    await figer(h.page);
    await appliquerLayout(h.page, arbre);
    return h;
}

/**
 * Déclare une scène de T1 : un coffre monté une fois sur un layout vide, puis un test par thème
 * (sombre, clair) qui joue `corps` et écrit reference/<nom>-<thème>.png.
 * `arbre` = le layout de départ (défaut : une pile de 4 onglets vides, le 2e actif).
 */
export function sceneT1(nom: string, corps: (c: ContexteT1) => Promise<void>, arbre: ArbreScene = PILE_PAR_DEFAUT): void {
    test.describe.serial(`pixels T1 — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await monter(arbre);
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                // Chaque thème repart du même arbre : les scènes qui le modifient ne se contaminent pas.
                await appliquerLayout(h.page, arbre);
                await schema(h.page, theme);
                await calme(h.page);
                const p = h.page;
                let maintenu: Bouton | null = null;
                const act: ActionsT1 = {
                    command: (id) => p.evaluate((i) => (window as any).app.commands.executeCommandById(i), id).then(() => {}),
                    layout: (a) => appliquerLayout(p, a),
                    click: (x, y, o = {}) => (o.page ?? p).mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 }),
                    hover: (x, y, page = p) => page.mouse.move(x, y),
                    key: (t) => p.keyboard.press(t),
                    type: (t) => p.keyboard.type(t),
                    wait: (ms) => p.waitForTimeout(ms),
                    waitStable: (page = p) => calme(page),
                    drag: async (from, to, o = {}) => {
                        const bouton = o.button ?? 'left';
                        await p.mouse.move(from.x, from.y);
                        await p.mouse.down({ button: bouton });
                        maintenu = bouton;
                        await p.mouse.move(to.x, to.y, { steps: o.steps ?? 10 });
                        if (o.release ?? true) {
                            await p.mouse.up({ button: bouton });
                            maintenu = null;
                        }
                    },
                    relacher: async () => {
                        if (maintenu) await p.mouse.up({ button: maintenu });
                        maintenu = null;
                    },
                };
                await corps({
                    h, p, theme, act,
                    capturer: async (page = p) => {
                        const png = await page.screenshot({ animations: 'disabled', caret: 'hide' });
                        writeFileSync(path.join(REFERENCE, `${nom}-${theme}.png`), png);
                    },
                    sonde: async (label, page, selecteur, n = 0) => {
                        const b = await page.locator(selecteur).nth(n).boundingBox();
                        if (!b) return;
                        mkdirSync(SONDES, { recursive: true });
                        appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene: nom, theme, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2), box: b }) + '\n');
                    },
                });
                // Un glisser laissé en cours ne doit pas fuir sur le thème suivant.
                if (maintenu) await p.mouse.up({ button: maintenu });
            });
        }
    });
}

export { expect };
