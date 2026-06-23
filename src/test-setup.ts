import { vi } from "vitest";
import "$lib/i18n";

// Create a global mock for $app/state before any imports
let mockPathname = "/";

const pageMock = {
  get url() {
    return { pathname: mockPathname };
  }
};

// Stub global for use in tests
vi.stubGlobal("__setMockPathname", (pathname: string) => {
  mockPathname = pathname;
});

// Mock the module
vi.mock("$app/state", () => ({
  page: pageMock
}));

vi.mock("$app/navigation", () => ({
  goto: vi.fn()
}));
