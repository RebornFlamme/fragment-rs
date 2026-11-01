// Port de app/e2e/visual-diff.spec.ts — scène « base » (sombre ET clair).
import { scene } from './_commun';

scene('base', async ({ capturer }) => {
    await capturer();
});
