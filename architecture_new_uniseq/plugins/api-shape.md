# Plugin API Shape

The plugin API should expose app-level concepts, not engine internals.

Example shape:

```ts
app.commands.register(...)
app.views.registerPanel(...)
app.workspace.search(...)
app.journal.append(...)
app.pages.getView(...)
app.tasks.query(...)
app.events.on("entry.changed", ...)
```

## Rule

Plugins should issue commands and subscribe to events.

They should not mutate indexes, caches, or files directly.

## Popular plugin patterns to support

- task dashboards
- quick capture
- calendar views
- query/report panels
- flashcard generation
- PDF helpers
- graph visualizers
- automation commands
- custom side panels
- theme and UI customization
