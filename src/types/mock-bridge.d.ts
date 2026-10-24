declare module "@mock-bridge" {
  export function mockCommand(name: string, payload: unknown): Promise<unknown>;
}
