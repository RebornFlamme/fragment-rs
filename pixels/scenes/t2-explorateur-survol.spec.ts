// Scène T2 « t2-explorateur-survol » : le pointeur sur « Chapitre 2 » (ligne survolée).
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-survol', async ({ act, capturer }) => {
    await act.hover(150, 221);
    await act.wait(200);
    await capturer();
});
