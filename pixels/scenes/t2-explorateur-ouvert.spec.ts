// Scène T2 « t2-explorateur-ouvert » : « Projets » ouvert, puis son sous-dossier « Alpha » (deux niveaux
// d'imbrication). Les clics visent le milieu des lignes (x = 150, pas de 26 px depuis y = 91).
import { sceneT2 } from './_t2';

sceneT2('t2-explorateur-ouvert', async ({ act, capturer, sonde, p }) => {
    await act.click(150, 117);          // Projets
    await act.click(150, 143);          // Alpha (première ligne sous Projets)
    await act.hover(2, 2);
    await act.waitStable();
    for (let i = 0; i < 14; i++) await sonde(`ligne ${i + 1}`, p, '.tree-item-self[data-path]', i);
    await capturer();
});
