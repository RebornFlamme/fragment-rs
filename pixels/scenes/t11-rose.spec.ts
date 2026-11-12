// Scène T11 « t11-rose » : le thème de coffre « Rose » posé sur le décor de T2 (explorateur + pile vide),
// avec le menu contextuel du dossier « Projets » ouvert : la palette du thème couvre tout l'écran.
import { sceneT11 } from './_t11';

sceneT11('t11-rose', async ({ act, capturer, sonde, choisirTheme, p }) => {
    await choisirTheme('Rose');
    await act.click(150, 117, { button: 'right' });
    await p.waitForSelector('.menu', { timeout: 3_000 });
    await sonde('menu', p, '.menu');
    await act.wait(200);
    await capturer();
});
