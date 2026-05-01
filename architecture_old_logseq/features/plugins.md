# Plugins

The plugin system is a major extension point.

## Capabilities

- startup and host lifecycle hooks
- slash commands
- UI items
- resources
- settings and config
- themes
- services
- simple commands

## Design choice

Plugins extend the app, but they do not own the document model.

That keeps the core product stable while still allowing customization.

