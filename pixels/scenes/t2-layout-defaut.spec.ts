// Scène T2 « t2-layout-defaut » : le premier démarrage sur un coffre sans workspace.json. Le layout par
// défaut de Fragment : l'explorateur à gauche, la première note au centre. ★ La note est une vue de
// fichier (T3) : le JSON porte « depuis »: « T3 ».
import { sceneT2 } from './_t2';

sceneT2('t2-layout-defaut', async ({ act, capturer, sonde, p }) => {
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('dock gauche', p, '.workspace-sidedock.mod-left');
    await capturer();
}, { layout: 'defaut' });
