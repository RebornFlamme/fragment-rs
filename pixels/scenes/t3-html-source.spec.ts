// Scène T3 « t3-html-source » : la source HTML du bloc, révélée par le bouton « </> » (retour à la source).
import { sceneT3c } from './_t3c';

sceneT3c('t3-html-source', 'html-blocs.md', async ({ act, capturer }) => {
    await act.hover(600, 262);
    await act.wait(200);
    // Un vrai clic sur le bouton du coin (centre 984, 257, mesuré dans t3-html-bouton).
    await act.click(984, 257);
    await act.figerSvg();
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
