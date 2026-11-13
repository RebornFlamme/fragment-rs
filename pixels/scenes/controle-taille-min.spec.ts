// Contrôle du mode invisible (fr-278) : fenêtre à la taille minimale EXACTE de Fragment (600 × 400).
// Preuve sans référence : rien n'est écrit dans pixels/reference (voir pixels/invisible/controle-taille.ts).
import { controleTaille } from '../invisible/controle-taille';

controleTaille('controle-taille-min', 600, 400);
