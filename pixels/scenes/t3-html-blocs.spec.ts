// Scène T3 « t3-html-blocs » : div stylée, SVG figé à t = 0, details fermé ; aucune source révélée.
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-blocs', 'html-blocs.md', async ({ act, p, capturer, sonde }) => {
    await act.figerSvg();
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('bloc', '.cm-html-block', 0);
    await capturer();
});
