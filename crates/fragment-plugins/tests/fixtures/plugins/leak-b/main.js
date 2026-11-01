// Lit le global posé par leak-a.
export default class LeakB {
  onload(fragment) {
    fragment.log("B|voit|" + typeof globalThis.__fuite + "|" + String(globalThis.__fuite));
  }
}
