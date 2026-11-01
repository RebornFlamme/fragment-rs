// Port de app/e2e/visual-diff.spec.ts — scène « selection » (sombre ET clair).
import { scene, calme } from './_commun';

scene('selection', async ({ p, sonde, capturer }) => {
    // Barre de sélection (Ask / Copier) sur un mot : double-clic dans le dernier paragraphe.
    const ligne = p.locator('.doc-editor .cm-content:visible .cm-line', { hasText: 'Un dernier paragraphe' });
    await sonde('dernier paragraphe', p, '.doc-editor .cm-content:visible .cm-line:has-text("Un dernier paragraphe")');
    await ligne.dblclick();
    await p.waitForTimeout(700);
    await calme(p);
    await capturer();
    await p.keyboard.press('Escape');
});
