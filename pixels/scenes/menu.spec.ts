// Port de app/e2e/visual-diff.spec.ts — scène « menu » (sombre ET clair).
import { scene, expect } from './_commun';

scene('menu', async ({ p, sonde, capturer }) => {
    // Menu contextuel dans la note, survol de la 2e entrée.
    await sonde('ligne 2 de la note', p, '.doc-editor .cm-content:visible .cm-line', 1);
    await p.locator('.doc-editor .cm-content:visible .cm-line').nth(1).click({ button: 'right' });
    await expect(p.locator('.menu').first()).toBeVisible();
    await sonde('entrée 2 du menu', p, '.menu .menu-item', 1);
    await p.locator('.menu .menu-item').nth(1).hover();
    await p.waitForTimeout(200);
    await capturer();
    await p.keyboard.press('Escape');
});
