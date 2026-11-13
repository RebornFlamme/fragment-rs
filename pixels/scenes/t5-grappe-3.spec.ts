// Scène T5 « t5-grappe-3 » : trois widgets repliés au même endroit : une tuile de trois bulles.
import { sceneT5a, expect } from './_t5a';

sceneT5a('t5-grappe-3', async ({ act, capturer, p }) => {
    await act.grappe('lighthouse', 3);
    await expect(p.locator('.doc-widget:has(.doc-widget-cluster) .doc-widget-icons .view-icon:not(.view-icon-more)')).toHaveCount(3);
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
