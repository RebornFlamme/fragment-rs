// ═══════════════════════════════════════════════════════════════════════════
//  Socle commun des scènes de référence pixel. Reprend le décor de
//  Fragment/app/e2e/visual-diff.spec.ts (note riche + PDF en split + chat +
//  conversation ancrée), mais :
//   - le coffre est pixels/vault/ (copié en mémoire, puis écrit par le harnais
//     dans un dossier TEMPORAIRE : le coffre du dépôt n'est jamais touché) ;
//   - la fenêtre fait 1280 × 800 à DPR 1 ;
//   - chaque capture est écrite dans pixels/reference/<scène>-<thème>.png.
//  Ce fichier ne contient aucun test (pas de suffixe .spec.ts).
// ═══════════════════════════════════════════════════════════════════════════
import { test, expect, type Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { appendFileSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'fs';
import path from 'path';
import { lancerLecteur, type Harnais } from '../../../Fragment/app/e2e/readerHarness';

const RACINE = path.resolve(__dirname, '..');
const COFFRE = path.join(RACINE, 'vault');
const REFERENCE = path.join(RACINE, 'reference');
const SONDES = path.join(RACINE, '.out');

export type Theme = 'dark' | 'light';
export interface Contexte {
    h: Harnais;
    p: Page;
    theme: Theme;
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

/** Aucune barre de défilement en cours d'apparition, aucun focus « au hasard ». */
export async function calme(page: Page): Promise<void> {
    await page.mouse.move(2, 2);
    await page.evaluate(() => document.fonts.ready);
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 150)))));
}

export async function schema(page: Page, s: Theme): Promise<void> {
    await page.evaluate((v) => (window as any).app.appearance.set('baseColorScheme', v), s);
    await page.waitForFunction((v) => document.documentElement.dataset.theme === v, s);
}

/** Le décor de visual-diff.spec.ts (beforeAll), sur le coffre pixels/vault/. */
async function monter(): Promise<Harnais> {
    const h = await lancerLecteur(lireCoffre(), 'note.md', { args: ['--force-device-scale-factor=1', '--high-dpi-support=1'] });
    await h.electronApp.evaluate(({ BrowserWindow }) => {
        for (const w of BrowserWindow.getAllWindows()) w.setContentSize(1280, 800);
    });
    await figer(h.page);
    await h.page.evaluate(async () => {
        const app = (window as any).app;
        if (!app.plugins.plugins.has('e2e-chat')) await app.plugins.enablePlugin('e2e-chat');
        const pdf = app.workspace.getLeaf('split');
        await pdf.openFile(app.vault.getFileByPath('doc.pdf'));
        await app.commands.executeCommandById('chat:new');
    });
    await expect(h.page.locator('.pdf-page canvas').first()).toBeVisible({ timeout: 15_000 });
    await h.page.locator('.chat-composer-input:visible').fill('Bonjour');
    await h.page.locator('.chat-composer-input:visible').press('Enter');
    await expect(h.page.locator('.chat-message[data-role="assistant"]').last()).toHaveAttribute('data-state', 'idle', { timeout: 10_000 });
    // Une conversation ANCRÉE dans la note (doc-widget, ouverte).
    await h.page.evaluate(() => {
        const app = (window as any).app;
        const note = app.workspace.getLeavesOfType('markdown').find((l: any) => l.getRoot() === app.workspace.rootSplit);
        app.workspace.setActiveLeaf(note, { focus: true });
        const from = note.view.getEditor().getValue().indexOf('dernier paragraphe');
        const host = app.registry.get({ id: 'doc-widget:host@1', kind: 'keyed' }).entries().at(-1).value;
        return host.open({ viewState: { type: 'chat', state: { provider: 'e2e', conversation: 'ancree' } }, file: 'note.md', from, to: from + 18, open: true });
    });
    await h.page.waitForTimeout(800);
    // Une conversation créée demande le focus pendant 2 s : attendre, puis le poser dans la note.
    await h.page.waitForTimeout(2500);
    await h.page.locator('.doc-editor .cm-content:visible .cm-line', { hasText: 'Un dernier paragraphe' }).click({ position: { x: 1, y: 2 } });
    await h.page.waitForTimeout(500);
    return h;
}

/**
 * Déclare une scène : un coffre monté une fois, puis un test par thème
 * (sombre, clair) qui joue `corps` et écrit reference/<nom>-<thème>.png.
 */
export function scene(nom: string, corps: (c: Contexte) => Promise<void>): void {
    test.describe.serial(`pixels — ${nom}`, () => {
        test.setTimeout(240_000);
        let h: Harnais;
        test.beforeAll(async () => {
            test.setTimeout(240_000);
            mkdirSync(REFERENCE, { recursive: true });
            h = await monter();
        });
        test.afterAll(async () => { await h?.electronApp.close(); });

        for (const theme of ['dark', 'light'] as const) {
            test(`${nom} — ${theme}`, async () => {
                await schema(h.page, theme);
                await calme(h.page);
                await corps({
                    h, p: h.page, theme,
                    capturer: async (page = h.page) => {
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
            });
        }
    });
}

export { expect };

/** Ouvre la fenêtre de réglages sur un onglet, la capture, la ferme. */
export async function reglages(c: Contexte, onglet: string): Promise<void> {
    const [r] = await Promise.all([
        c.h.electronApp.waitForEvent('window', { timeout: 20_000 }),
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
    await c.capturer(r);
    await r.close();
}
