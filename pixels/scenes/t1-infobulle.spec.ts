// Scène T1 « t1-infobulle » : l'infobulle d'un petit élément (placement par défaut : dessous).
import { sceneFlottante } from './_t1-flottantes';

sceneFlottante('t1-infobulle', async ({ p, act, capturer, sonde }) => {
    await p.evaluate(async () => {
        const { setTooltip, displayTooltip } = await import('/src/ui/core/tooltip.ts' as string);
        const cible = document.body.appendChild(document.createElement('div'));
        cible.className = 'pixels-cible';
        cible.style.cssText = 'position:fixed;left:300px;top:200px;width:40px;height:20px';
        setTooltip(cible, 'Calques');
        displayTooltip(cible);
    });
    await sonde('infobulle', p, '.tooltip');
    await act.waitStable();
    await capturer();
});
