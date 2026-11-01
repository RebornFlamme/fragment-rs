// Port de app/e2e/visual-diff.spec.ts — scène « palette » (sombre ET clair).
import { scene, calme, expect } from './_commun';

scene('palette', async ({ p, capturer }) => {
    // Palette de commandes, requête « zoom ».
    await p.evaluate(() => (window as any).app.commands.executeCommandById('command-palette:open'));
    await expect(p.locator('.modal-container').first()).toBeVisible();
    await p.keyboard.type('zoom');
    await calme(p);
    await capturer();
    await p.keyboard.press('Escape');
});
