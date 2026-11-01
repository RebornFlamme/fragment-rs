// Port de app/e2e/visual-diff.spec.ts — scène « fenêtre de réglages, onglet Editor » (sombre ET clair).
import { scene, reglages } from './_commun';

scene('reglages-editor', async (c) => {
    await reglages(c, 'Editor');
});
