# Pages

Pages are the primary knowledge objects in the workspace.

Their views combine persisted page content with derived incoming references and related sections.

## Default page layout

- page title and optional description
- page markdown content when the page has its own file
- pinned entries
- incoming journal entries grouped by date
- open tasks related to the page
- related pages and tags

## Page navigation

Pages should be navigable through a clear hierarchy in the main shell.

The default mental model should be close to Notion:

- expandable page tree
- parent/child nesting where the user curates it
- obvious location and breadcrumb context
- page discovery without needing a separate "all pages" dumping view

The hierarchy and the derived sections belong to the same page model:

- hierarchy determines where the page appears in the tree
- the page file stores only the page's own content and metadata
- incoming references and related sections are shown by the frontend and are not stored back into the page file

The page tree is not a filesystem folder tree.

If the app supports parent/child hierarchy, it should be modeled at the page level rather than inferred from disk folders.

## Page creation

A page may first appear because a user creates it directly, writes a tag, writes a page link, or imports content.

These are not different kinds of pages. They are just different ways the same page can first enter the workspace.

New pages default to top-level placement unless namespace or explicit hierarchy places them elsewhere.

The app should not require a page file for every page.
