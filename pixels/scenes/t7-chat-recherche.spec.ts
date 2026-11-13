// Scène T7 « t7-chat-recherche » : Ctrl+F dans le fil, « crue » saisi : barre, compte, surlignages.
import { sceneT7a, expect, MAINTENANT, UNE_FEUILLE } from './_t7a';

sceneT7a('t7-chat-recherche', async ({ act, capturer, p }) => {
    await act.horloge(MAINTENANT);
    await act.layout(UNE_FEUILLE);
    await act.chat('page', 'c-long');
    await act.cliquer('.chat-view[data-mode="page"] .chat-composer-input');
    // ★ Mod+f vit dans la portée de la vue chat ; une portée persistante restée au sommet de la pile la masque :
    //   on dépile ces portées parasites (jamais la racine ni la vue), puis Ctrl+F ouvre la barre.
    await p.evaluate(() => {
        const app = (window as any).app; const km = app.keymap; const vue = app.workspace.activeLeaf.view.scope;
        while (km.scope !== km.rootScope && km.scope !== vue) km.popScope(km.scope);
    });
    await act.key('Control+f');
    await expect(p.locator('.chat-search')).toBeVisible();
    await act.type('crue');
    await expect(p.locator('.chat-search-count')).not.toHaveText('');
    await act.hover(2, 2);
    await act.waitStable();
    await capturer();
});
