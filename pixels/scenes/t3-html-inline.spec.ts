// Scène T3 « t3-html-inline » : span, mark, u, sub et br rendus, balises cachées.
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-inline', 'html-inline.md', async ({ act, p, capturer, sonde }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
