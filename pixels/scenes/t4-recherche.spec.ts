// Scène T4 « t4-recherche » : Ctrl+F, « balise » saisi : barre, compte, surlignages (le résultat courant en orange).
import { sceneT4 } from './_t4';

sceneT4('t4-recherche', async ({ act, capturer, p }) => {
    // ★ Mod+f vit dans la portée de la vue PDF. Or une portée PERSISTANTE (celle d'une barre d'outils
    //   restée à l'écran, parent = racine) est au sommet de la pile clavier et masque celle de la vue :
    //   Ctrl+F n'atteint jamais PdfView. On dépile ces portées parasites (jamais la racine ni la vue),
    //   sans toucher au DOM, puis on laisse Ctrl+F ouvrir la barre et donner le focus au champ.
    await p.evaluate(() => {
        const app = (window as any).app; const km = app.keymap; const vue = app.workspace.getLeavesOfType('pdf')[0].view.scope;
        while (km.scope !== km.rootScope && km.scope !== vue) km.popScope(km.scope);
    });
    await act.key('Control+f');
    await p.locator('.pdf-find-bar.is-open').waitFor();
    await p.waitForFunction(() => document.activeElement?.classList.contains('pdf-find-input'));
    await act.type('balise');
    await p.locator('.pdf-find-count').filter({ hasText: / of 5$/ }).waitFor();
    await p.waitForTimeout(500);
    await act.waitStable();
    await capturer();
});
