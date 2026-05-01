# Plugin Runtime

Plugins should run in an isolated JS runtime or controlled browser context.

## Requirements

- manifest file
- declared permissions
- command registration
- panel registration
- event subscription
- plugin-local storage
- controlled network access
- uninstall and disable flows

## Host responsibilities

The TypeScript app shell owns the plugin host UI.

The Rust engine owns capability enforcement for durable operations.

