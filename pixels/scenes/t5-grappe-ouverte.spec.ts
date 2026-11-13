// Scène T5 « t5-grappe-ouverte » : clic sur la tuile de quatre membres : la vue de grappe s'ouvre EN PLACE (liste des
// membres), les bulles ne bougent pas. Le pointeur est ensuite garé hors de la grappe.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-ouverte', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 4);
    await act.cliquer('.doc-widget-icons');
    await expect(p.locator('.doc-widget .doc-widget-cluster')).toBeVisible();
    await expect(p.locator('.doc-widget-cluster-item')).toHaveCount(4);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
