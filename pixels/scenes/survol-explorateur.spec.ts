// Port de app/e2e/visual-diff.spec.ts — scène « survol-explorateur » (sombre ET clair).
import { scene, expect } from './_commun';

scene('survol-explorateur', async ({ p, sonde, capturer }) => {
    // Survol d'un fichier de l'explorateur (le premier visible).
    const item = p.locator('.nav-file-title:visible').first();
    expect(await item.count()).toBeGreaterThan(0);
    await sonde('premier fichier', p, '.nav-file-title:visible');
    await item.hover();
    await p.waitForTimeout(200);
    await capturer();
});
