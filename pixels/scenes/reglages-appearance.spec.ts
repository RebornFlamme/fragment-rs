// Port de app/e2e/visual-diff.spec.ts — scène « fenêtre de réglages, onglet Appearance » (sombre ET clair).
import { scene, reglages } from './_commun';

scene('reglages-appearance', async (c) => {
    await reglages(c, 'Appearance');
});
