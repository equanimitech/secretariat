import '@testing-library/jest-dom'
import { vi } from 'vitest'

// Mock matchMedia for tests
Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: vi.fn().mockImplementation(query => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: vi.fn(), // deprecated
    removeListener: vi.fn(), // deprecated
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
})

// Mock Tauri APIs for tests
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {
    // Mock unlisten function
  }),
}))

// Mock the window API so hooks that observe fullscreen state don't blow
// up in jsdom. `onResized` returns a Promise<UnlistenFn> in real Tauri.
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    isFullscreen: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(() => {
      // Mock unlisten function
    }),
    onFocusChanged: vi.fn().mockResolvedValue(() => {
      // Mock unlisten function
    }),
  })),
}))

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn().mockResolvedValue(null),
}))

// Mock secretariat-specific bindings (tauri-specta generated)
vi.mock('@/lib/bindings', () => ({
  commands: {
    getProfile: vi.fn().mockResolvedValue({ status: 'ok', data: null }),
    currentIdentity: vi.fn().mockResolvedValue({ status: 'ok', data: null }),
  },
}))

// Mock typed Tauri bindings (tauri-specta generated)
vi.mock('@/lib/tauri-bindings', () => ({
  commands: {
    greet: vi.fn().mockResolvedValue('Hello, test!'),
    loadPreferences: vi
      .fn()
      .mockResolvedValue({ status: 'ok', data: { theme: 'system' } }),
    savePreferences: vi.fn().mockResolvedValue({ status: 'ok', data: null }),
    sendNativeNotification: vi
      .fn()
      .mockResolvedValue({ status: 'ok', data: null }),
    saveEmergencyData: vi.fn().mockResolvedValue({ status: 'ok', data: null }),
    loadEmergencyData: vi.fn().mockResolvedValue({ status: 'ok', data: null }),
    cleanupOldRecoveryFiles: vi
      .fn()
      .mockResolvedValue({ status: 'ok', data: 0 }),
    takePendingOpens: vi.fn().mockResolvedValue([]),
    openMarkdownWindow: vi
      .fn()
      .mockResolvedValue({ status: 'ok', data: 'md:test' }),
    readMarkdown: vi
      .fn()
      .mockResolvedValue({
        status: 'ok',
        data: { content: '', sha256: '0'.repeat(64) },
      }),
    writeMarkdown: vi.fn().mockResolvedValue({
      status: 'ok',
      data: { kind: 'ok', sha256: '0'.repeat(64) },
    }),
  },
  unwrapResult: vi.fn((result: { status: string; data?: unknown }) => {
    if (result.status === 'ok') return result.data
    throw result
  }),
}))
