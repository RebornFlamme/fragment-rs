// Scène T1 « t1-activation-fermeture » : on ferme l'onglet ACTIF (le 2e) par sa croix : l'onglet
// qui prend sa place (l'ancien 3e) devient actif et visible. Reste 3 onglets, le 2e actif.
// ★ Le clic passe par `click()` du DOM : `page.mouse` ne pilote pas les en-têtes dans ce harnais.
import { sceneT1 } from './_t1';

sceneT1('t1-activation-fermeture', async ({ act, capturer, sonde, p }) => {
    await sonde('croix de l\'onglet actif', p, '.workspace-tab-header.is-active .workspace-tab-header-inner-close-button');
    await p.evaluate(() => {
        (document.querySelector('.workspace-tab-header.is-active .workspace-tab-header-inner-close-button') as HTMLElement).click();
    });
    await act.waitStable();
    await sonde('onglet actif après', p, '.workspace-tab-header.is-active');
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
});
