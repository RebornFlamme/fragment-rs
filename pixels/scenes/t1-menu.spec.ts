// Scène T1 « t1-menu » : menu à sections (ordre déclaré, entrée ajoutée en retard), séparateurs,
// entrée désactivée, entrée à sous-menu, entrée destructive ; le survol de l'entrée 2 est posé mais Chromium ne le peint pas dans ce harnais (cause non élucidée) :
// le JSON candidat garde donc le pointeur garé, pour que les deux côtés montrent la même chose.
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-menu', async ({ p, act, capturer, sonde }) => {
    await p.evaluate(async () => {
        const { Menu } = await import('/src/ui/core/Menu.ts' as string);
        const menu = new Menu();
        menu.addSections(['edition', 'fichier', 'danger']);
        menu.addItem((i: any) => i.setTitle('Copier').setIcon('copy').setSection('edition'));
        menu.addItem((i: any) => i.setTitle('Coller').setSection('edition'));
        menu.addItem((i: any) => i.setTitle('Renommer').setSection('fichier').setDisabled(true));
        menu.addItem((i: any) => {
            i.setTitle('Trier par').setSection('fichier');
            i.setSubmenu().addItem((s: any) => s.setTitle('Nom'));
        });
        menu.addItem((i: any) => i.setTitle('Supprimer').setIcon('trash-2').setSection('danger').setWarning(true));
        // Ajoutée APRÈS : elle atterrit tout de même dans le groupe « edition ».
        menu.addItem((i: any) => i.setTitle('Dupliquer').setSection('edition'));
        menu.showAtPosition(420, 260);
    });
    await sonde('entrée 2 du menu', p, '.menu .menu-item', 1);
    await sonde('menu', p, '.menu');
    await act.hover(470, 295);
    await act.waitStable();
    await capturer();
});
