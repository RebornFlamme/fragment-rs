// Scène T5 « t5-grappe-plus » : six widgets repliés au même endroit : le disque, trois bulles et « +3 » dans la grande bas-droite.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-plus', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 6);
    await expect(p.locator('.doc-widget-icons .view-icon-more')).toHaveText('+3');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
