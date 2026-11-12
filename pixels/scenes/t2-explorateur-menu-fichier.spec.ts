// Scène T2 « t2-explorateur-menu-fichier » : le menu contextuel d'un fichier (« Journal.md »).
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-menu-fichier', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 273, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', p, '.menu');
    await act.wait(200);
    await capturer();
});
