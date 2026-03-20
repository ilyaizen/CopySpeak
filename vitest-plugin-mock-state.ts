import type { Plugin } from 'vite';

// Mock for $app/state - virtual module
let mockPathname = '/';

export const mockAppStatePlugin = (): Plugin => ({
  name: 'mock-app-state',
  resolveId(source) {
    if (source === '$app/state') {
      return '\0mock-app-state';
    }
    return null;
  },
  load(id) {
    if (id === '\0mock-app-state') {
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
    return null;
  },
});

// Export function to update pathname from tests
export function setMockPathname(pathname: string) {
  mockPathname = pathname;
}
