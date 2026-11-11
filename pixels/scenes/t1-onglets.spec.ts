// Scène T1 « t1-onglets » : quatre onglets de vues vides, le 2e actif (décor de base de T1).
// Les sondes notent les positions des en-têtes pour écrire le JSON candidat.
import { sceneT1 } from './_t1';

sceneT1('t1-onglets', async ({ act, capturer, sonde, p }) => {
    for (let i = 0; i < 4; i++) await sonde(`onglet ${i + 1}`, p, '.workspace-tab-header', i);
    await sonde('onglet actif', p, '.workspace-tab-header.is-active');
    await sonde('croix de l\'onglet actif', p, '.workspace-tab-header.is-active .workspace-tab-header-inner-close-button');
    await sonde('bouton +', p, '.workspace-tab-header-new-tab .clickable-icon');
    await sonde('liste des onglets', p, '.workspace-tab-header-tab-list .clickable-icon');
    await sonde('barre', p, '.workspace-tab-header-container');
    await act.waitStable();
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
});
