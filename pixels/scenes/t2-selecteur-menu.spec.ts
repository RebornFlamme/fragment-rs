// Scène T2 « t2-selecteur-menu » : le menu du sélecteur de coffre (barre du pied du dock gauche) :
// le coffre courant coché, puis « Gérer les coffres… ». Le JSON de cette scène est écrit par
// fr-t2-gestionnaire (côté candidat : la commande pixels:vault-switcher-menu, T2.md §5).
import { sceneT2 } from './_t2';

sceneT2('t2-selecteur-menu', async ({ act, capturer, sonde, p }) => {
    await act.click(80, 780);
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', p, '.menu');
    await act.wait(200);
    await capturer();
});
