// Port de app/e2e/visual-diff.spec.ts — scène « notice » (sombre ET clair).
import { scene, calme } from './_commun';

scene('notice', async ({ p, capturer }) => {
    // Une notice persistante (durée 0).
    await p.evaluate(async () => {
        const mod = await import('/src/ui/core/Notice.ts' as string);
        new mod.Notice('Une notice de test', 0);
    });
    await p.waitForTimeout(400);
    await calme(p);
    await capturer();
    await p.evaluate(() => document.querySelectorAll('.notice').forEach((n) => n.remove()));
});
