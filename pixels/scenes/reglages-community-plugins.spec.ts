// Port de app/e2e/visual-diff.spec.ts — scène « fenêtre de réglages, onglet Community plugins » (sombre ET clair).
import { scene, reglages } from './_commun';

scene('reglages-community-plugins', async (c) => {
    await reglages(c, 'Community plugins');
});
