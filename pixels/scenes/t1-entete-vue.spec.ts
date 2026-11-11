// Scène T1 « t1-entete-vue » : l'en-tête de la vue en grille à trois zones (← → grisés, titre vide
// pour une vue vide, « ⋯ » à droite) et le message de la vue vide. Le pointeur est sur le bouton
// « ⋯ » (fond de survol).
// ★ Une feuille neuve ou restaurée vide porte une vue vide JAMAIS MONTÉE : un panneau blanc, sans
// en-tête (voir `t1-onglets`). C'est `setViewState` qui la monte (en-tête + « Aucun fichier ouvert ») :
// la scène le fait pour la feuille visible.
import { sceneT1 } from './_t1';

sceneT1('t1-entete-vue', async ({ act, capturer, sonde, p }) => {
    await p.evaluate(async () => {
        const f = (window as any).app.workspace.getLeavesOfType('empty')[1];
        await f.setViewState({ type: 'empty', state: {} });
    });
    await act.waitStable();
    await sonde('en-tête de vue', p, '.view-header');
    await sonde('flèche arrière', p, '.view-header .view-action', 0);
    await sonde('flèche avant', p, '.view-header .view-action', 1);
    await sonde('plus d\'options', p, '.view-header .view-action', 2);
    await sonde('message', p, '.empty-state');
    const b = await p.locator('.view-header .view-action').nth(2).boundingBox();
    if (!b) throw new Error('bouton ⋯ introuvable');
    await act.hover(Math.round(b.x + b.width / 2), Math.round(b.y + b.height / 2));
    await act.wait(300);
    await capturer();
});
