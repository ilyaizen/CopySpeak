import type { Plugin } from "vite";

// Mock for SvelteKit virtual modules used in vitest
let mockPathname = "/";

export const mockAppStatePlugin = (): Plugin => ({
  name: "mock-app-state",
  resolveId(source) {
    if (source === "$app/state") {
      return "\0mock-app-state";
    }
    if (source === "$app/navigation") {
      return "\0mock-app-navigation";
    }
    return null;
  },
  load(id) {
    if (id === "\0mock-app-state") {
      return `
        export const page = {
          get url() {
            return { pathname: '${mockPathname}' };
          },
          setPathname(pathname) {
            if (typeof global !== 'undefined' && global.__setMockPathname) {
              global.__setMockPathname(pathname);
            }
          }
        };
      `;
    }
    if (id === "\0mock-app-navigation") {
      return `
        export function goto() {
          return Promise.resolve();
        }
      `;
    }
    return null;
  }
});

// Export function to update pathname from tests
export function setMockPathname(pathname: string) {
  mockPathname = pathname;
}
