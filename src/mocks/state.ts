// Virtual mock for $app/state used in vitest
let mockPathname = "/";

export const page = {
  get url() {
    return { pathname: mockPathname };
  },
  setPathname(pathname: string) {
    mockPathname = pathname;
  }
};

// Allow tests to set pathname
// SAFETY: only adds __setMockPathname; no other global members are touched or hidden.
(
  globalThis as typeof globalThis & { __setMockPathname?: (pathname: string) => void }
).__setMockPathname = (pathname: string) => {
  mockPathname = pathname;
};
