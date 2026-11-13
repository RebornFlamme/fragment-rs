// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T5, groupe A (marge, arbitre, grappes), côté Fragment (référence pixel).
//
//  Une NOTE ouverte seule dans une feuille (le coffre pixels/vault-t5/a : une note écrite par nous,
//  en ASCII pur, pour que les positions de caractères soient les mêmes en UTF-16 qu'en octets), docks
//  repliés, caret masqué, rendu figé. Les noms d'actions sont ceux du JSON de fragment-pixels.
//
//  ★ Les widgets sont posés par les FIXTURES de Fragment (plugins de test e2e-room, e2e-globe,
//    e2e-dwv), EXÉCUTÉES depuis le dépôt de Fragment et jamais recopiées (ADR-007) : le harnais les
//    écrit dans le coffre temporaire. Côté candidat, le JSON dit les commandes `pixels:widget-test` et
//    `pixels:doc-widget` du contrat de docs/tranches/T5.md §5 ; ce sont les widgets de test Rust
//    (rigide, flex, seuil, vue nue) qui dessinent la même chose.
//  ★ Correspondance fixture ↔ contrat :
//      e2e-globe (`__globePos`, `__globeHauteur`)  ↔ kind « rigide », `from`, `height`
//      e2e-room, mode flex | seuil (`pos`, `natural`) ↔ kind « flex » | « seuil », `from`, `height`
//      e2e-dwv (vue nue `e2e-room-view`, via le service doc-widget:host@1) ↔ `pixels:doc-widget`
//    `side` vaut toujours « right » (les fixtures ne posent que dans la marge droite).
//  ★ `id` de `pixels:activer` : « w<rang> », le rang étant celui de la commande `pixels:widget-test`
//    dans la scène (0 pour la première). La référence active le widget par un VRAI clic.
//  ★ Le JSON du candidat est écrit par le même code que la référence (.out/actions/<scène>.json, lu
//    au thème sombre), jamais réestimé à la main.
//  ★ `wait-stable` ne déplace PAS le pointeur (README de pixels/) : la scène de survol garde le sien
//    sur la tuile ; les autres le garent par un `hover 2,2` explicite.
//  ★ Texte tel que l'utilisateur le voit (fr-p4.1.4, ADR-017) : ClearType sur fond opaque, gris dans un calque.
//
//  Aucun test ici (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { calme, schema, type Theme } from './_commun';

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t5', 'a');
const FIXTURES = path.resolve(RACINE, '..', '..', 'Fragment', 'app', 'e2e', 'fixtures');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');
const ACTIONS = path.join(SONDES, 'actions');

const NOTE = 'note.md';
export const ACTIONS_VUE = ['split', 'tab', 'dock', 'collapse'];

export interface Point { x: number; y: number }
type Action = Record<string, unknown>;

export interface OptionsT5a {
    /** Fenêtre de la scène (défaut 1280 × 800). */
    fenetre?: { largeur: number; hauteur: number };
    /** Le pointeur reste où la scène l'a laissé (pas de `hover 2,2` final). */
    garerPointeur?: boolean;
}

export interface WidgetTest {
    kind: 'flex' | 'seuil';
    from: number;
    height: number;
}

export interface ActionsT5a {
    /** `command pixels:widget-test` : le globe rigide (calque e2e-globe), PUIS les widgets du calque e2e-room, dans l'ordre. */
    poser(spec: { rigide?: { from: number; height: number }; widgets?: WidgetTest[] }): Promise<void>;
    /** `command pixels:activer` : la référence clique le widget `rang` (rang parmi les widgets du calque e2e-room). */
    activer(rang: number): Promise<void>;
    /** `command pixels:doc-widget` : `n` vues nues REPLIÉES ancrées sur le mot `mot` (`largeur` caractères chacune, 2 par défaut, de proche en proche). */
    grappe(mot: string, n: number, largeur?: number): Promise<void>;
    click(x: number, y: number): Promise<void>;
    /** Clique au centre de l'élément (arrondi au pixel, même point dans le JSON). */
    cliquer(selecteur: string, nth?: number): Promise<void>;
    /** Déplace le pointeur au centre de l'élément (arrondi au pixel, même point dans le JSON). */
    survoler(selecteur: string, nth?: number): Promise<void>;
    hover(x: number, y: number): Promise<void>;
    wait(ms: number): Promise<void>;
    waitStable(): Promise<void>;
}

export interface ContexteT5a {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT5a;
    /** Le début du paragraphe `n` (1 = le titre) de la note, en caractères. */
    pos(n: number): number;
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

/** Un plugin de test de Fragment, lu depuis son dépôt (exécuté, pas recopié), posé dans le coffre temporaire. */
function plugin(id: string, fichier: string): Record<string, Buffer | string> {
    return {
        [`.fragment/plugins/${id}/manifest.json`]: JSON.stringify({ id, name: id, version: '1.0.0' }),
        [`.fragment/plugins/${id}/main.js`]: readFileSync(path.join(FIXTURES, fichier)),
    };
}

async function figer(page: Page): Promise<void> {
    await page.emulateMedia({ reducedMotion: 'reduce' }).catch(() => {});
    await page.addStyleTag({
        content: '*,*::before,*::after{caret-color:transparent!important;animation:none!important;transition:none!important;scroll-behavior:auto!important}',
    });
}

const vide = () => ({ type: 'split', direction: 'vertical', children: [], collapsed: true });

/** Le layout du décor : une pile centrale, UNE feuille portant la note, docks vides repliés. */
function layoutNote(): unknown {
    const feuille = { type: 'leaf', id: 'pixels-t5a-note', state: { type: 'markdown', state: { file: NOTE } } };
    return {
        version: 1,
        main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', currentTab: 0, children: [feuille] }] },
        left: vide(),
        right: vide(),
        active: 'pixels-t5a-note',
    };
}

async function monter(fenetre: { largeur: number; hauteur: number }): Promise<Harnais> {
    const fichiers = {
        ...lireCoffre(),
        ...plugin('e2e-room', 'gutter-room-plugin.cjs'),
        ...plugin('e2e-globe', 'gutter-globe-plugin.cjs'),
        ...plugin('e2e-dwv', 'doc-widget-view-plugin.cjs'),
    };
    const h = await lancerLecteur(fichiers, NOTE, { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
    await h.electronApp.evaluate(({ BrowserWindow }, f) => {
        for (const w of BrowserWindow.getAllWindows()) w.setContentSize(f.largeur, f.hauteur);
    }, fenetre);
    await figer(h.page);
    await h.page.evaluate(async () => {
        const app = (window as any).app;
        for (const id of ['e2e-room', 'e2e-globe', 'e2e-dwv']) if (!app.plugins.plugins.has(id)) await app.plugins.enablePlugin(id);
    });
    return h;
}

/** Remet le décor à zéro : plus aucun widget ancré, puis la note ouverte seule dans un layout neuf, en haut. */
async function ouvrirNote(p: Page): Promise<void> {
    await p.evaluate(() => {
        const app = (window as any).app;
        for (const r of app.artefacts.list({ type: 'doc-widget:pin' })) app.artefacts.del(r.id, 'user');
    });
    await p.evaluate((l) => (window as any).app.workspace.changeLayout(l), layoutNote() as any);
    await p.waitForSelector('.doc-editor > .cm-editor .cm-content', { timeout: 20_000 });
    await p.evaluate(() => {
        // Les calques de test ne sont posés que par la scène (e2e-globe est actif par défaut à l'ouverture).
        const v = (window as any).app.workspace.activeLeaf.view;
        for (const id of ['e2e-room', 'e2e-globe']) if (v.isLayerActive(id)) v.toggleLayer(id);
        (window as any).__room = [];
        (document.activeElement as HTMLElement | null)?.blur();
    });
    await p.waitForTimeout(300);
}

/** Polices prêtes, deux images, 150 ms, SANS bouger le pointeur (le `wait-stable` du JSON). */
async function stable(p: Page): Promise<void> {
    await p.evaluate(() => document.fonts.ready);
    await p.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 150)))));
}

/** Les boîtes des widgets de marge ne bougent plus (l'arbitre a fini). */
async function placesStables(p: Page, selecteur: string): Promise<void> {
    let precedent = '';
    for (let i = 0; i < 20; i++) {
        await p.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => setTimeout(r, 0))));
        const boites = JSON.stringify(await p.locator(selecteur).evaluateAll((els) => els.map((e) => {
            const r = e.getBoundingClientRect();
            return [r.x, r.y, r.width, r.height];
        })));
        if (boites === precedent) return;
        precedent = boites;
        await p.waitForTimeout(120);
    }
}

export function sceneT5a(nom: string, corps: (c: ContexteT5a) => Promise<void>, options: OptionsT5a = {}): void {
    const fenetre = options.fenetre ?? { largeur: 1280, hauteur: 800 };
    const note = readFileSync(path.join(COFFRE, NOTE), 'utf8');
    // Le début du paragraphe n (séparés par une ligne vide), 1-based.
    const pos = (n: number): number => {
        let p = 0;
        for (let i = 1; i < n; i++) p = note.indexOf('\n\n', p) + 2;
        return p;
    };
    test.describe.serial(`pixels T5a — ${nom}`, () => {
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
                await ouvrirNote(p);
                await schema(p, theme);
                await calme(p);
                const log: Action[] = [];
                const noter = (a: Action) => { log.push(a); };
                const commande = (id: string, args: unknown) => noter({ type: 'command', id, args });
                let rangs = 0;
                const centre = async (sel: string, nth = 0): Promise<Point> => {
                    const b = await p.locator(sel).nth(nth).boundingBox();
                    if (!b) throw new Error(`élément introuvable : ${sel}[${nth}]`);
                    return { x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2) };
                };
                const act: ActionsT5a = {
                    poser: async ({ rigide, widgets = [] }) => {
                        await p.evaluate(({ rigide, widgets }) => {
                            const w = window as any;
                            const v = w.app.workspace.activeLeaf.view;
                            w.__room = widgets.map((x: any) => ({ pos: x.from, natural: x.height, mode: x.kind }));
                            if (rigide) { w.__globePos = rigide.from; w.__globeHauteur = rigide.height; }
                            if (rigide) v.toggleLayer('e2e-globe');
                            if (widgets.length) v.toggleLayer('e2e-room');
                        }, { rigide, widgets });
                        if (rigide) {
                            await expect(p.locator('.e2e-globe')).toHaveCount(1);
                            commande('pixels:widget-test', { kind: 'rigide', from: rigide.from, side: 'right', height: rigide.height });
                            rangs++;
                        }
                        await expect(p.locator('.e2e-room')).toHaveCount(widgets.length);
                        for (const x of widgets) {
                            commande('pixels:widget-test', { kind: x.kind, from: x.from, side: 'right', height: x.height });
                            rangs++;
                        }
                        await placesStables(p, '.e2e-room, .e2e-globe');
                    },
                    activer: async (rang) => {
                        // Le VRAI geste : un clic sur le widget (activation après le geste).
                        const { x, y } = await centre('.e2e-room', rang);
                        await p.mouse.click(x, y);
                        // Le rang dans le JSON = rang de commande (le globe éventuel passe avant les widgets).
                        const decalage = log.filter((a) => (a.args as any)?.kind === 'rigide').length;
                        commande('pixels:activer', { id: `w${rang + decalage}` });
                        await placesStables(p, '.e2e-room, .e2e-globe');
                    },
                    grappe: async (mot, n, largeur = 2) => {
                        const bases = await p.evaluate(({ mot, n, largeur, ACTIONS_VUE }) => {
                            const app = (window as any).app;
                            (window as any).__dwActions = ACTIONS_VUE;
                            const feuille = app.workspace.getLeavesOfType('markdown').find((l: any) => l.getRoot() === app.workspace.rootSplit);
                            const base = feuille.view.getEditor().getValue().indexOf(mot);
                            const hote = app.registry.get({ id: 'doc-widget:host@1', kind: 'keyed' }).entries().at(-1).value;
                            return Array.from({ length: n }, (_, i) => {
                                const from = base + i * largeur;
                                hote.open({ viewState: { type: 'e2e-room-view', state: {} }, file: 'note.md', from, to: from + largeur, open: false });
                                return from;
                            });
                        }, { mot, n, largeur, ACTIONS_VUE });
                        for (const from of bases) commande('pixels:doc-widget', { from, to: from + largeur, open: false, actions: ACTIONS_VUE });
                        await expect(p.locator('.doc-widget.is-gutter-clustered')).toHaveCount(n);
                        await placesStables(p, '.doc-widget');
                    },
                    click: async (x, y) => {
                        await p.mouse.click(x, y);
                        noter({ type: 'click', x, y });
                    },
                    cliquer: async (sel, nth = 0) => {
                        const { x, y } = await centre(sel, nth);
                        await act.click(x, y);
                    },
                    survoler: async (sel, nth = 0) => {
                        const { x, y } = await centre(sel, nth);
                        await act.hover(x, y);
                    },
                    hover: async (x, y) => { await p.mouse.move(x, y); noter({ type: 'hover', x, y }); },
                    wait: async (ms) => { await p.waitForTimeout(ms); noter({ type: 'wait', ms }); },
                    waitStable: async () => { await stable(p); noter({ type: 'wait-stable' }); },
                };
                await corps({
                    h, p, theme, act, pos,
                    capturer: async () => {
                        const png = await p.screenshot({ animations: 'disabled', caret: 'hide' });
                        writeFileSync(path.join(REFERENCE, `${nom}-${theme}.png`), png);
                    },
                    sonde: async (label, selecteur, n = 0) => {
                        const b = await p.locator(selecteur).nth(n).boundingBox();
                        if (!b) return;
                        mkdirSync(SONDES, { recursive: true });
                        appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene: nom, theme, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2), box: b }) + '\n');
                    },
                });
                // Le JSON du candidat : la note ouverte seule, puis les gestes joués, puis (sauf demande) le pointeur garé.
                if (theme === 'dark') {
                    const fin: Action[] = options.garerPointeur === false ? [] : [{ type: 'hover', x: 2, y: 2 }, { type: 'wait-stable' }];
                    const actions = [
                        { type: 'layout', tree: { main: { type: 'tabs', tabs: 1, current: 0 } } },
                        { type: 'open', path: NOTE },
                        { type: 'wait-stable' },
                        ...log,
                        ...fin,
                    ];
                    mkdirSync(ACTIONS, { recursive: true });
                    const lignes = actions.map((a) => '    ' + JSON.stringify(a));
                    writeFileSync(path.join(ACTIONS, `${nom}.json`), [
                        '{',
                        `  "scene": ${JSON.stringify(nom)},`,
                        '  "depuis": "T5",',
                        `  "window": {"width":${fenetre.largeur},"height":${fenetre.hauteur},"dpr":1},`,
                        '  "themes": ["dark","light"],',
                        '  "vault": "pixels/vault-t5/a",',
                        '  "actions": [',
                        lignes.join(',\n'),
                        '  ]',
                        '}',
                        '',
                    ].join('\n'));
                }
            });
        }
    });
}

export { expect };
