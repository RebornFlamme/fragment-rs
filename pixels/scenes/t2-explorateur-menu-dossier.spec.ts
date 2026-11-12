// Scène T2 « t2-explorateur-menu-dossier » : le menu contextuel d'un dossier (« Projets »).
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-menu-dossier', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 117, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', p, '.menu');
    await act.wait(200);
    await capturer();
});
