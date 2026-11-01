// Port de app/e2e/visual-diff.spec.ts — scène « annotation » (sombre ET clair).
import { scene, calme } from './_commun';

scene('annotation', async ({ p, capturer }) => {
    // Barre d'annotation.
    await p.evaluate(() => (window as any).app.commands.executeCommandById('annotation:toggle'));
    await p.waitForTimeout(400);
    await calme(p);
    await capturer();
    await p.evaluate(() => (window as any).app.commands.executeCommandById('annotation:toggle'));
    await p.keyboard.press('Escape');
});
