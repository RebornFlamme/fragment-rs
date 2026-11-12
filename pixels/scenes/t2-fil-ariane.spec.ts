// Scène T2 « t2-fil-ariane » : « Projets/Alpha/cahier.md » ouvert, le fil d'Ariane de l'en-tête de vue
// (un segment par dossier). ★ Exige une vue de fichier (T3) : « depuis »: « T3 ».
import { sceneT2 } from './_t2';

sceneT2('t2-fil-ariane', async ({ act, capturer, sonde, p }) => {
    await p.evaluate(async () => {
        const app = (window as any).app;
        await app.workspace.getLeaf(false).openFile(app.vault.getFileByPath('Projets/Alpha/cahier.md'), { active: true });
    });
    await act.wait(500);
    await act.hover(2, 2);
    await act.waitStable();
    await sonde('en-tête', p, '.workspace-tabs.mod-active .view-header, .view-header');
    await capturer();
});
