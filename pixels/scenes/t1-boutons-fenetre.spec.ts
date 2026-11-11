// Scène T1 « t1-boutons-fenetre » : un pane ÉTROIT (part 1 contre 10) au coin haut-droit, moins
// large que les trois boutons de fenêtre. L'espace des boutons va à la pile qui est DESSOUS (la
// grande, à gauche), qui ne réserve que le recouvrement et montre le bouton du dock droit.
import { sceneT1 } from './_t1';

sceneT1('t1-boutons-fenetre', async ({ act, capturer, sonde, p }) => {
    await act.waitStable();
    // ★ Le harnais pose les dimensions APRÈS le dernier calcul de l'espace des boutons : sans ce
    // `resize` (écouté par Workspace, qui recalcule l'espace), il restait sur le pane étroit.
    await p.evaluate(() => window.dispatchEvent(new Event('resize')));
    await act.waitStable();
    for (let i = 0; i < 2; i++) await sonde(`pile ${i + 1}`, p, '.workspace-tabs', i);
    await sonde('bouton du dock droit', p, '.workspace-tab-header-right-toggle');
    await sonde('boutons de fenêtre', p, '.titlebar-button-container');
    await act.hover(640, 500);
    await act.wait(150);
    await capturer();
}, { main: { type: 'split', direction: 'vertical', children: [{ type: 'tabs', tabs: 2, current: 0, dimension: 10 }, { type: 'tabs', tabs: 1, dimension: 1 }] } });
