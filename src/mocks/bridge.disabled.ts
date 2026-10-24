/**
 * Production stub for the browser mock bridge. The real mock module is excluded
 * from release bundles by the Vite alias; even if the import survived, this
 * stub cannot provide data.
 */
export function mockCommand(_name: string, _payload: unknown): Promise<unknown> {
  return Promise.reject(new Error("mock bridge is unavailable"));
}
