import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Component tests exercise the browser mock bridge, never the native IPC.
vi.stubEnv("VITE_MOCK_BRIDGE", "1");
