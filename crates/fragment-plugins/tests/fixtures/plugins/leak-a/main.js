// Pose un global : voit-on la trace depuis un AUTRE plugin ? (isolation du TAS, pas des capacités)
export default class LeakA {
  onload(fragment) {
    try {
      globalThis.__fuite = "secret-de-A";
      fragment.log("A|pose|" + globalThis.__fuite);
    } catch (e) {
      fragment.log("A|refuse|" + e.message);
    }
  }
}
