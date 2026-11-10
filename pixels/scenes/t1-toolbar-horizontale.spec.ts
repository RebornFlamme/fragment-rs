// Scène T1 « t1-toolbar-horizontale » : la barre par défaut (horizontale), sans sections : un outil
// actif, un séparateur manuel, une action désactivée, la poignée au bout.
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-toolbar-horizontale', async ({ p, act, capturer, sonde }) => {
    await p.evaluate(async () => {
        const { Toolbar } = await import('/src/ui/core/Toolbar.ts' as string);
        const bar = new Toolbar();
        bar.addItem((i: any) => i.setIcon('pencil').setTooltip('Crayon').setGroup('outil').setActive(true));
        bar.addItem((i: any) => i.setIcon('eraser').setTooltip('Gomme').setGroup('outil'));
        bar.addItem((i: any) => i.setIcon('highlighter').setTooltip('Surligneur').setGroup('outil'));
        bar.addSeparator();
        bar.addItem((i: any) => i.setIcon('undo-2').setTooltip('Annuler'));
        bar.addItem((i: any) => i.setIcon('redo-2').setTooltip('Rétablir').setDisabled(true));
        bar.showAtPosition(300, 250);
    });
    await sonde('barre', p, '.toolbar');
    await act.waitStable();
    await capturer();
});
