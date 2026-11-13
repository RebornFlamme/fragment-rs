// ═══════════════════════════════════════════════════════════════════════════
//  Décor des scènes de T3, groupe B (code, séparateur, maths, tableaux, Properties, alignement,
//  colonne étroite, lecture seule), côté Fragment (référence pixel).
//
//  Le coffre pixels/vault-t3/b (fixture à nous, notes écrites de zéro) est monté par le harnais du
//  lecteur. Le layout est ensuite REMPLACÉ : une seule pile au centre, portant UNE note ouverte,
//  docks vides et repliés, caret masqué, rendu figé. La fenêtre fait 1280 × 800 (600 × 400 pour
//  la colonne étroite, la taille minimale de Fragment). Les noms d'actions sont ceux du JSON de
//  fragment-pixels (voir _t1.ts).
//
//  ★ Un curseur se pose par un clic aux MÊMES coordonnées des deux côtés ; la lecture seule se
//  met par un clic sur le bouton réel de l'en-tête de vue (pas de commande dédiée) ; un menu de
//  propriété s'ouvre par un clic sur son icône, puis un survol de l'entrée.
//
//  Le harnais de Fragment est IMPORTÉ par chemin, jamais recopié (ADR-007). Ce fichier n'a aucun
//  test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';
import { calme, schema, type Theme } from './_commun';

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault-t3', 'b');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

export type Bouton = 'left' | 'right' | 'middle';

export interface ActionsT3b {
    /** `click {x,y,button,count}`. */
    click(x: number, y: number, options?: { button?: Bouton; count?: number }): Promise<void>;
    hover(x: number, y: number): Promise<void>;
    key(touches: string): Promise<void>;
    type(texte: string): Promise<void>;
    wait(ms: number): Promise<void>;
    /** `wait-stable` : polices prêtes, 2 images, 150 ms, SANS bouger le pointeur. */
    waitStable(): Promise<void>;
}

export interface ContexteT3b {
    h: Harnais;
    p: Page;
    theme: Theme;
    act: ActionsT3b;
    capturer(): Promise<void>;
    /** Note le centre d'un élément dans .out/coords.jsonl (pour écrire le JSON candidat). */
    sonde(label: string, selecteur: string, nth?: number): Promise<void>;
}

export interface OptionsT3b {
    /** Taille de contenu de la fenêtre (défaut 1280 × 800). */
    largeur?: number;
    hauteur?: number;
}

/** Le coffre pixels/vault-t3/b en mémoire : { 'x.md': Buffer }. */
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
    const feuille = { type: 'leaf', id: 'pixels-t3b-note', state: { type: 'markdown', state: { file: fichier } } };
    return {
        version: 1,
        main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', currentTab: 0, children: [feuille] }] },
        left: vide(),
        right: vide(),
        active: 'pixels-t3b-note',
    };
}

/** Ouvre `fichier` dans une feuille neuve (l'éditeur est monté, l'état d'une passe précédente a disparu). */
async function ouvrirNote(p: Page, fichier: string): Promise<void> {
    await p.evaluate((l) => (window as any).app.workspace.changeLayout(l), layoutNote(fichier) as any);
    await p.waitForSelector('.doc-editor > .cm-editor .cm-content', { timeout: 20_000 });
    await p.waitForTimeout(300);
}

/**
 * Déclare une scène de T3 groupe B : un coffre monté une fois, puis un test par thème (sombre, clair)
 * qui rouvre `fichier` dans un layout neuf, joue `corps` et écrit reference/<nom>-<thème>.png.
 */
export function sceneT3b(nom: string, fichier: string, corps: (c: ContexteT3b) => Promise<void>, options: OptionsT3b = {}): void {
    const { largeur = 1280, hauteur = 800 } = options;
    test.describe.serial(`pixels T3b — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await lancerLecteur(lireCoffre(), 'alignement.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
            await h.electronApp.evaluate(({ BrowserWindow }, [w, hh]) => {
                for (const f of BrowserWindow.getAllWindows()) f.setContentSize(w, hh);
            }, [largeur, hauteur]);
            await figer(h.page);
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                const p = h.page;
                await ouvrirNote(p, fichier);
                await schema(p, theme);
                await calme(p);
                const act: ActionsT3b = {
                    click: (x, y, o = {}) => p.mouse.click(x, y, { button: o.button ?? 'left', clickCount: o.count ?? 1 }),
                    hover: (x, y) => p.mouse.move(x, y),
                    key: (t) => p.keyboard.press(t),
                    type: (t) => p.keyboard.type(t),
                    wait: (ms) => p.waitForTimeout(ms),
                    waitStable: () => calme(p),
                };
                await corps({
                    h, p, theme, act,
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
            });
        }
    });
}
