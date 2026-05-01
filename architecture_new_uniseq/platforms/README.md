# Platform Architecture

The app should support desktop, browser, and mobile.

The product model and Rust engine stay as shared as possible across targets.

Platform-specific layers adapt:

- filesystem access
- permissions
- lifecycle
- background behavior
- platform-native integrations

Recommended targets:

- Tauri for desktop
- web browser target with web-friendly filesystem/storage APIs
- mobile wrapper with a native bridge such as Capacitor
