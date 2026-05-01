# Runtime

The runtime layer owns startup and orchestration.

It is responsible for:

- booting the app
- restoring repositories
- wiring handlers
- routing to the current page
- initializing plugins and UI listeners
- running background loops

Keep this layer thin. Its job is to connect subsystems, not implement them.

