// Scène T5 « t5-grappe-2 » : deux widgets repliés tombés au même endroit : UNE tuile de deux bulles (la 1re icône de chaque vue), sans fond.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-2', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 2);
    await expect(p.locator('.doc-widget:has(.doc-widget-cluster) .doc-widget-icons .view-icon:not(.view-icon-more)')).toHaveCount(2);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
