// ═══════════════════════════════════════════════════════════════════════════
//  Les scènes de réglages et de zoom de T1, côté Fragment (référence pixel).
//
//  La fenêtre de réglages est une 2e fenêtre Electron (1000 × 700) : on l'ouvre par
//  `app.setting.open()`, on attend son rail, on clique l'onglet, et on la capture à part. Le zoom de
//  Chromium est PAR ORIGINE : tout zoom posé pendant une scène doit être rendu (`rendreLeZoom`), sinon
//  il fuit sur le thème suivant (et sur les autres scènes du même coffre).
//
//  ★ NON RÉÉCRIT d'après _commun.ts : le décor note + PDF + chat n'a rien à faire ici, la fenêtre de
//  réglages ne le voit pas (elle a son propre App). La fonction `reglages` de _commun.ts sert de
//  modèle pour l'ouverture et le gel.
// ═══════════════════════════════════════════════════════════════════════════
import type { Page } from '../../../Fragment/app/node_modules/@playwright/test';
import { expect, type ContexteT1 } from './_t1';

const FIGE = '*,*::before,*::after{caret-color:transparent!important;animation:none!important;transition:none!important;scroll-behavior:auto!important}';

/** Ouvre la fenêtre de réglages sur un onglet : 1000 × 700, rendu figé, pointeur garé. */
export async function ouvrirReglages(c: ContexteT1, onglet: string): Promise<Page> {
    const [r] = await Promise.all([
        c.h.electronApp.waitForEvent('window', { timeout: 20_000 }),
        c.p.evaluate(() => (window as any).app.setting.open()),
    ]);
    await r.waitForFunction(() => !!document.querySelector('.vertical-tab-nav-item'), undefined, { timeout: 20_000 });
    await r.setViewportSize({ width: 1000, height: 700 }).catch(() => {});
    await r.emulateMedia({ reducedMotion: 'reduce' }).catch(() => {});
    await r.addStyleTag({ content: FIGE });
    const nav = r.locator('.vertical-tab-nav-item', { hasText: onglet }).first();
    expect(await nav.count(), `onglet « ${onglet} » absent`).toBeGreaterThan(0);
    await c.sonde(`onglet ${onglet}`, r, `.vertical-tab-nav-item:has-text("${onglet}")`);
    await nav.click();
    await calmer(r);
    return r;
}

/** Polices prêtes, deux images, pointeur garé dans le coin. */
export async function calmer(page: Page): Promise<void> {
    await page.mouse.move(2, 2);
    await page.evaluate(() => document.fonts.ready);
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(r, 150)))));
}

/** Le zoom EFFECTIF de la fenêtre (facteur). */
export const zoomFenetre = (p: Page): Promise<number> => p.evaluate(() => (window as any).app.windowHost.getZoom());

/** Attend que le zoom effectif atteigne `facteur` (le main applique en différé). */
export async function attendreZoom(p: Page, facteur: number): Promise<void> {
    await expect.poll(() => zoomFenetre(p), { timeout: 10_000 }).toBeCloseTo(facteur, 2);
}

/** Remet le zoom à 100 % (commande `app:reset-zoom`) et attend la fenêtre. */
export async function rendreLeZoom(c: ContexteT1): Promise<void> {
    await c.act.command('app:reset-zoom');
    await attendreZoom(c.p, 1);
}
