// Port de app/e2e/visual-diff.spec.ts — scène « fenêtre de réglages, onglet Hotkeys » (sombre ET clair).
import { scene, reglages } from './_commun';

scene('reglages-hotkeys', async (c) => {
    await reglages(c, 'Hotkeys');
});
