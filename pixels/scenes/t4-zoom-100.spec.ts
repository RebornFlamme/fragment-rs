// Scène T4 « t4-zoom-100 » : taille réelle (mode page-actual : 1 px par point PDF).
import { sceneT4 } from './_t4';

sceneT4('t4-zoom-100', async ({ act, capturer, p }) => {
    await act.zoom({ mode: 'page-actual' });
    await act.waitStable();
    await capturer();
});
