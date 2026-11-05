// Per-tab state behind one store object (docs/specs/document-tabs.md).
// The editors' modules read and write their stores as `store.key`; with
// several documents open, the per-document keys belong to the active tab.
// `tabStore` returns a proxy that forwards those keys to the object
// `active()` returns (the active tab's own `$state`) and every other key to
// `shared`, so the modules keep their imports while each tab owns its
// document, view and file state. Both objects are Svelte `$state`, and
// `active()` reads reactive state, so a tab switch re-runs every reader.

/**
 * A store whose `keys` live on `active()` and whose other keys live on
 * `shared`. Writes follow the same split.
 */
export function tabStore<S extends object, D extends object>(
  shared: S,
  keys: readonly (keyof D & string)[],
  active: () => D,
): S & D {
  const own = new Set<PropertyKey>(keys);
  return new Proxy(shared, {
    get(target, key) {
      return own.has(key) ? Reflect.get(active(), key) : Reflect.get(target, key);
    },
    set(target, key, value) {
      return own.has(key) ? Reflect.set(active(), key, value) : Reflect.set(target, key, value);
    },
    has(target, key) {
      return own.has(key) || Reflect.has(target, key);
    },
  }) as S & D;
}
