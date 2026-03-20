// Virtual mock for $app/state used in vitest
let mockPathname = '/';

export const page = {
  get url() {
    return { pathname: mockPathname };
  },
  setPathname(pathname: string) {
    mockPathname = pathname;
  },
};

// Allow tests to set pathname
(globalThis as Record<string, unknown>).__setMockPathname = (pathname: string) => {
  mockPathname = pathname;
};
