// Scène T4 « t4-zoom-200 » : le PDF à 200 % (la page déborde, défilement horizontal possible).
import { sceneT4 } from './_t4';

sceneT4('t4-zoom-200', async ({ act, capturer, p }) => {
    await act.zoom({ valeur: 2 });
    await act.waitStable();
    await capturer();
});
