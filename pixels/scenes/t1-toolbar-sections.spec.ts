// Scène T1 « t1-toolbar-sections » : sections déclarées (outil, style, agent), items ajoutés dans le
// désordre, séparateurs régénérés entre les groupes ; un item avec libellé.
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-toolbar-sections', async ({ p, act, capturer, sonde }) => {
    await p.evaluate(async () => {
        const { Toolbar } = await import('/src/ui/core/Toolbar.ts' as string);
        const bar = new Toolbar();
        bar.addSections(['outil', 'style', 'agent']);
        bar.addItem((i: any) => i.setIcon('palette').setTooltip('Couleur').setSection('style'));
        bar.addItem((i: any) => i.setIcon('sparkles').setLabel('Demander').setTooltip('Demander').setSection('agent'));
        bar.addItem((i: any) => i.setIcon('pencil').setTooltip('Crayon').setSection('outil').setGroup('outil').setActive(true));
        bar.addItem((i: any) => i.setIcon('eraser').setTooltip('Gomme').setSection('outil').setGroup('outil'));
        bar.showAtPosition(300, 250);
    });
    await sonde('barre', p, '.toolbar');
    await act.waitStable();
    await capturer();
});
