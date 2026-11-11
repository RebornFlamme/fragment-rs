// Scène T1 « t1-reglages-appearance » : la fenêtre de réglages (1000 × 700), onglet Appearance, zoom à 100 %.
// Reprend `reglages-appearance` (visual-diff) sans le décor note + PDF + chat : la fenêtre de réglages ne le voit pas.
import { sceneT1 } from './_t1';
import { ouvrirReglages } from './_t1-reglages';

sceneT1('t1-reglages-appearance', async (c) => {
    const r = await ouvrirReglages(c, 'Appearance');
    await c.capturer(r);
    await r.close();
});
