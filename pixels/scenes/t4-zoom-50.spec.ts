// Scène T4 « t4-zoom-50 » : le PDF à 50 % (échelle libre, choisie dans le menu des modes).
import { sceneT4 } from './_t4';

sceneT4('t4-zoom-50', async ({ act, capturer, p }) => {
    await act.zoom({ valeur: 0.5 });
    await act.waitStable();
    await capturer();
});
