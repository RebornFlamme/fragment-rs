// Scène T5 « t5-grappe-survol » : le pointeur sur la tuile de quatre membres : les passages de TOUS les membres se
// surlignent dans la note et l'infobulle « 4 éléments » s'affiche à droite. Le pointeur reste sur la tuile.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-survol', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 4, 6);
    await act.survoler('.doc-widget-icons');
    await expect(p.locator('.range-highlight.doc-widget-passage')).toHaveCount(4);
    await expect(p.locator('.tooltip.mod-right')).toHaveText('4 éléments');
    await act.wait(800);
    await act.waitStable();
    await capturer();
}, { garerPointeur: false });
