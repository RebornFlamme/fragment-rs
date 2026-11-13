// Scène T5 « t5-grappe-disque » : cinq membres : le disque de la maquette, trois bulles et « +2 ».
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-disque', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 5);
    await expect(p.locator('.doc-widget-icons .view-icon-more')).toHaveText('+2');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
