// Scène T2 « t2-entete-nav-actif » : les flèches ← → de l'en-tête de vue TOUTES DEUX actives. Trois
// fichiers ouverts à la suite dans la même feuille, puis un retour en arrière : il y a un « avant » et
// un « après ». ★ Exige des vues de fichier (T3) : « depuis »: « T3 ».
import { sceneT2 } from './_t2';

sceneT2('t2-entete-nav-actif', async ({ act, capturer, sonde, p }) => {
    await p.evaluate(async () => {
        const app = (window as any).app;
        for (const chemin of ['Accueil.md', 'Journal.md', 'zebre.md']) {
            await app.workspace.getLeaf(false).openFile(app.vault.getFileByPath(chemin), { active: true });
        }
    });
    await act.wait(300);
    await act.command('app:go-back');
    await act.wait(500);
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('en-tête', p, '.workspace-tabs.mod-active .view-header, .view-header');
    await capturer();
});
