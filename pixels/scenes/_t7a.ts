// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T7, groupe A (vue chat, cowork, fil, défilement), côté Fragment.
//
//  Le coffre pixels/vault-t7/a (une note et conversations.json) est monté dans l'app ; un plugin de
//  fixture (e2e-chat-t7) y inscrit un fournisseur de chat sans réseau, dont les conversations sont
//  celles de conversations.json (lu aussi par le candidat Rust). Le « faux agent » est un script
//  d'événements posé par la scène : rejoué d'un coup, puis figé (hold) pour capturer pendant le flux.
//  Aucun appel réel à l'agent, aucun prompt ni skill de Fragment.
//
//  ★ Horloge : `page.clock.setFixedTime` fige Date.now() (les minuteurs courent). Les groupes
//    « Aujourd'hui / Hier » de la barre et la salutation de l'accueil en dépendent : chaque scène
//    commence par `act.horloge(ISO)`, enregistré comme la commande `pixels:horloge`.
//  ★ Chaque geste passe par `act`, qui l'EXÉCUTE sur Fragment et l'ENREGISTRE sous la forme de
//    l'action JSON équivalente (schéma de pixels/README.md) : le JSON du candidat est écrit par le
//    même code que la référence (.out/actions/<scène>.json), jamais réestimé à la main.
//  ★ Texte tel que l'utilisateur le voit (fr-p4.1.4, ADR-017) : ClearType sur fond opaque, gris dans un calque.
//  ★ Le pointeur n'est garé (hover 2,2) que si la scène le demande : un survol (rail) ou un menu
//    doivent rester tels quels à la capture ; `wait-stable` ne bouge jamais le pointeur.
//
//  Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007). Pas de test ici.
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { schema, type Theme } from './_commun';
import { layoutVide, type ArbreScene } from './_t1';

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t7', 'a');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');
const ACTIONS = path.join(SONDES, 'actions');

/** Le jour des scènes : le 12 novembre 2026, 14 h 30 (heure locale) — « Bon après-midi, Julien ». */
export const MAINTENANT = '2026-11-12T14:30:00';

/** Une feuille, un onglet, docks repliés. */
export const UNE_FEUILLE: ArbreScene = { main: { type: 'tabs', tabs: 1, current: 0 } };
/** Une feuille et le dock droit ouvert (le chat en panneau). */
export const AVEC_DOCK_DROIT: ArbreScene = { main: { type: 'tabs', tabs: 1, current: 0 }, right: { collapsed: false, width: 380 } };

export interface Point { x: number; y: number }
export type Bouton = 'left' | 'right' | 'middle';
type Action = Record<string, unknown>;
export type ModeChat = 'panel' | 'page';

export interface OptionsT7a {
    /** Fenêtre de la scène (défaut 1280 × 800). */
    fenetre?: { largeur: number; hauteur: number };
    /** Garer le pointeur en fin de scène (défaut oui). Faux : survol ou menu à conserver. */
    garer?: boolean;
}

export interface ActionsT7a {
    /** `command pixels:horloge {iso}` : fige l'heure. */
    horloge(iso: string): Promise<void>;
    /** `layout {tree}`. */
    layout(arbre: ArbreScene): Promise<void>;
    /** `open {path}` : la note dans la feuille centrale. */
    open(chemin: string): Promise<void>;
    /** `command pixels:chat {mode, conversation}` : la conversation, en panneau (dock droit) ou en onglet. */
    chat(mode: ModeChat, conversation: string): Promise<void>;
    /** `command pixels:agent-script {conversation, events, hold}` : le prochain tour jouera ces événements. */
    agentScript(conversation: string, events: { type: 'text-delta'; text: string }[], hold: boolean): Promise<void>;
    /** `command pixels:chat-defiler {to}` : `fin` (pastille) ou l'id d'un message placé en haut. */
    defiler(to: string): Promise<void>;
    /** `command {id}` : une commande du CommandManager (ex. workspace:toggle-margin). */
    command(id: string): Promise<void>;
    click(x: number, y: number, options?: { button?: Bouton; count?: number }): Promise<void>;
    /** Clique au centre de l'élément (arrondi au pixel, même point dans le JSON). */
    cliquer(selecteur: string, nth?: number): Promise<void>;
    hover(x: number, y: number): Promise<void>;
    /** Survole le centre de l'élément. */
    survoler(selecteur: string, nth?: number): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    waitStable(): Promise<void>;
}

export interface ContexteT7a {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT7a;
    capturer(): Promise<void>;
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

/** Polices prêtes, deux images, 150 ms — SANS bouger le pointeur (comme `wait-stable`). */
async function stable(page: Page): Promise<void> {
    await page.evaluate(() => document.fonts.ready);
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 150)))));
}

async function lancer(fenetre: { largeur: number; hauteur: number }): Promise<Harnais> {
    const h = await lancerLecteur(lireCoffre(), 'carnet.md', {
        args: ['--force-device-scale-factor=1', '--high-dpi-support=1'],
    });
    await h.electronApp.evaluate(({ BrowserWindow }, f) => {
        for (const w of BrowserWindow.getAllWindows()) w.setContentSize(f.largeur, f.hauteur);
    }, fenetre);
    await figer(h.page);
    await h.page.evaluate(async () => {
        const app = (window as any).app;
        if (!app.plugins.plugins.has('e2e-chat-t7')) await app.plugins.enablePlugin('e2e-chat-t7');
    });
    return h;
}

export function sceneT7a(nom: string, corps: (c: ContexteT7a) => Promise<void>, options: OptionsT7a = {}): void {
    const fenetre = options.fenetre ?? { largeur: 1280, hauteur: 800 };
    const garer = options.garer ?? true;
    test.describe.serial(`pixels T7a — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await lancer(fenetre);
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                const p = h.page;
                await schema(p, theme);
                // État neuf : conversations relues, script oublié, pointeur garé, plus aucune feuille.
                await p.evaluate(() => (window as any).__t7.reset());
                await p.mouse.move(2, 2);
                const log: Action[] = [];
                const noter = (a: Action) => { log.push(a); };
                const centre = async (sel: string, nth = 0): Promise<Point> => {
                    const b = await p.locator(sel).nth(nth).boundingBox();
                    if (!b) throw new Error(`élément introuvable : ${sel}[${nth}]`);
                    return { x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2) };
                };
                const commande = (id: string, args?: unknown) => noter({ type: 'command', id, ...(args === undefined ? {} : { args }) });
                const act: ActionsT7a = {
                    horloge: async (iso) => {
                        await p.clock.setFixedTime(new Date(iso));
                        commande('pixels:horloge', { iso });
                    },
                    layout: async (arbre) => {
                        await p.evaluate((layout) => (window as any).app.workspace.changeLayout(layout), layoutVide(arbre) as any);
                        await p.waitForTimeout(200);
                        noter({ type: 'layout', tree: arbre });
                    },
                    open: async (chemin) => {
                        await p.evaluate(async (f) => {
                            const app = (window as any).app;
                            const feuille = app.workspace.getLeavesOfType('empty')[0];
                            await feuille.openFile(app.vault.getFileByPath(f), { active: true });
                        }, chemin);
                        await p.waitForSelector('.cm-content');
                        noter({ type: 'open', path: chemin });
                    },
                    chat: async (mode, conversation) => {
                        await p.evaluate(async ({ mode, conversation }) => {
                            const ws = (window as any).app.workspace;
                            const etat = { type: 'chat', state: { provider: 't7', conversation }, active: true };
                            if (mode === 'panel') {
                                await ws.getRightLeaf(false).setViewState(etat);
                                await ws.rightSplit.expand();
                            } else {
                                // L'onglet vide de la feuille centrale devient le chat.
                                await ws.getLeavesOfType('empty')[0].setViewState(etat);
                            }
                        }, { mode, conversation });
                        await p.waitForSelector(`.chat-view[data-mode="${mode}"]`);
                        const n = await p.evaluate((c) => (window as any).__t7.compte(c), conversation);
                        if (n) await expect(p.locator(`.chat-view[data-mode="${mode}"] .chat-message`)).toHaveCount(n, { timeout: 15_000 });
                        // Formules et tableaux se rendent après les messages.
                        await p.waitForTimeout(400);
                        commande('pixels:chat', { mode, conversation });
                    },
                    agentScript: async (conversation, events, hold) => {
                        await p.evaluate((s) => { (window as any).__t7.script = s; }, { events, hold });
                        commande('pixels:agent-script', { conversation, events, hold });
                    },
                    defiler: async (to) => {
                        await p.evaluate((cible) => {
                            const v = (window as any).app.workspace.activeLeaf.view;
                            if (cible === 'fin') v.scroll.jump();
                            else v.scroll.reveal(cible);
                        }, to);
                        await p.waitForTimeout(300);
                        commande('pixels:chat-defiler', { to });
                    },
                    command: async (id) => {
                        await p.evaluate((i) => (window as any).app.commands.executeCommandById(i), id);
                        await p.waitForTimeout(400);
                        noter({ type: 'command', id });
                    },
                    click: async (x, y, o = {}) => {
                        await p.mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 });
                        noter({ type: 'click', x, y, ...(o.button && o.button !== 'left' ? { button: o.button } : {}), ...(o.count && o.count > 1 ? { count: o.count } : {}) });
                    },
                    cliquer: async (sel, nth = 0) => {
                        const { x, y } = await centre(sel, nth);
                        await act.click(x, y);
                    },
                    hover: async (x, y) => { await p.mouse.move(x, y); noter({ type: 'hover', x, y }); },
                    survoler: async (sel, nth = 0) => {
                        const { x, y } = await centre(sel, nth);
                        await act.hover(x, y);
                    },
                    key: async (k) => { await p.keyboard.press(k); noter({ type: 'key', keys: k }); },
                    type: async (t) => { await p.keyboard.type(t); noter({ type: 'type', text: t }); },
                    wait: async (ms) => { await p.waitForTimeout(ms); noter({ type: 'wait', ms }); },
                    waitStable: async () => { await stable(p); noter({ type: 'wait-stable' }); },
                };
                await corps({
                    h, p, theme, act,
                    capturer: async () => {
                        const png = await p.screenshot({ animations: 'disabled', caret: 'hide' });
                        writeFileSync(path.join(REFERENCE, `${nom}-${theme}.png`), png);
                    },
                });
                // Le JSON du candidat : écrit une fois (thème sombre), depuis les gestes joués.
                if (theme === 'dark') {
                    // Déjà garé par la scène (hover 2,2 puis wait-stable) : rien à ajouter.
                    const garee = log.length >= 2 && JSON.stringify(log.slice(-2)) === JSON.stringify([{ type: 'hover', x: 2, y: 2 }, { type: 'wait-stable' }]);
                    const fin: Action[] = garer && !garee ? [{ type: 'hover', x: 2, y: 2 }, { type: 'wait-stable' }] : [];
                    const actions = [...log, ...fin];
                    const lignes = actions.map((a) => '    ' + JSON.stringify(a));
                    mkdirSync(ACTIONS, { recursive: true });
                    writeFileSync(
                        path.join(ACTIONS, `${nom}.json`),
                        `{\n  "scene": "${nom}",\n  "depuis": "T7",\n  "window": {"width":${fenetre.largeur},"height":${fenetre.hauteur},"dpr":1},\n  "themes": ["dark","light"],\n  "vault": "pixels/vault-t7/a",\n  "actions": [\n${lignes.join(',\n')}\n  ]\n}\n`,
                    );
                }
            });
        }
    });
}

/** Note le centre d'un élément dans .out/coords.jsonl (diagnostic). */
export async function sonde(p: Page, scene: string, label: string, selecteur: string, nth = 0): Promise<void> {
    const b = await p.locator(selecteur).nth(nth).boundingBox();
    if (!b) return;
    mkdirSync(SONDES, { recursive: true });
    appendFileSync(path.join(SONDES, 'coords.jsonl'), JSON.stringify({ scene, label, x: Math.round(b.x + b.width / 2), y: Math.round(b.y + b.height / 2) }) + '\n');
}

export { expect };
