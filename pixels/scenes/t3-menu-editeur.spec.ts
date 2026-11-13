// Scène T3 « t3-menu-editeur » : le menu contextuel de l'éditeur (clic droit sur une ligne).
import { sceneT3c } from './_t3c';

sceneT3c('t3-menu-editeur', 'menu.md', async ({ act, p, capturer, sonde }) => {
    await act.click(470, 253, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', '.menu');
    await act.wait(200);
    await capturer();
});
