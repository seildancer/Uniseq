# Focus and Zoom

Focus is an editing and reading affordance over the current parsed document.

It is not a durable block-identity feature.

## Supported focus modes

- open a page in its own full view
- temporarily isolate an entry or subtree while editing
- return from focused view back to normal page context

## Design rule

Focus state is ephemeral UI/editor state.

It may depend on the current parsed tree and current selection, but it does not require durable block UUIDs or block-native page identities.

## Non-goal

Focus should not imply:

- stable block identity across restarts
- durable block URLs as a core product primitive
- block ref / embed semantics
