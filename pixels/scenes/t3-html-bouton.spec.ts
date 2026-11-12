// Scène T3 « t3-html-bouton » : le bouton « </> » au coin du bloc survolé.
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-bouton', 'html-blocs.md', async ({ act, p, capturer, sonde }) => {
    await act.figerSvg();
    await act.hover(600, 262);
    await act.wait(200);
    await sonde('bouton', '.edit-block-button', 0);
    await capturer();
});
