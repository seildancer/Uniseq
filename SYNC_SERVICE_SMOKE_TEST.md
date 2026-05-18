# Uniseq Sync Service Smoke Test

This file is a minimal backend-author checklist for verifying compatibility with the current Uniseq client.

Use a real `SYNC_ROOT_URL`, an optional `TOKEN`, and a test workspace name such as `smoke-test`.

## Variables

Examples:

```text
SYNC_ROOT_URL=https://selfhosted.example.com/johndoe
TOKEN=your-token-here
WORKSPACE_NAME=smoke-test
```

If your service does not require auth, omit the `Authorization` header in the examples below.

## 1. Discovery

Request:

```http
GET {SYNC_ROOT_URL}/.well-known/uniseq-sync
```

Valid outcomes:

- `404 Not Found`
- empty body
- `null`
- JSON discovery document

Bearer example:

```json
{
  "version": 1,
  "auth": {
    "type": "bearer",
    "login_url": "https://selfhosted.example.com/login",
    "instructions": "Create a token and paste it into Uniseq."
  }
}
```

## 2. List Workspaces

Request:

```http
GET {SYNC_ROOT_URL}/workspaces
Authorization: Bearer {TOKEN}
```

Expect a JSON array:

```json
[]
```

or:

```json
[
  {
    "id": "personal",
    "name": "Personal",
    "updated_at": "2026-05-19T12:00:00Z"
  }
]
```

## 3. Create Workspace

Request:

```http
POST {SYNC_ROOT_URL}/workspaces
Authorization: Bearer {TOKEN}
Content-Type: application/json

{
  "name": "{WORKSPACE_NAME}"
}
```

Expect:

```json
{
  "id": "smoke-test",
  "name": "smoke-test",
  "updated_at": "2026-05-19T12:00:00Z"
}
```

Save the returned `id` as `WORKSPACE_ID`.

## 4. Upload a File

Request:

```http
PUT {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}/files/pages/A.md
Authorization: Bearer {TOKEN}

# Hello
```

Notes:

- The current client may omit `Content-Type` on `PUT`.
- A new file may omit `X-Uniseq-Base-Remote-Version`.

Expect:

```json
{
  "status": "accepted",
  "remote_version": "v1",
  "updated_at": "2026-05-19T12:01:00Z"
}
```

Save `remote_version` as `V1`.

## 5. List Files

Request:

```http
GET {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}/files
Authorization: Bearer {TOKEN}
```

Expect:

```json
[
  {
    "path": "pages/A.md",
    "remote_version": "v1",
    "size": 8,
    "updated_at": "2026-05-19T12:01:00Z"
  }
]
```

## 6. Pull the File

Either of these is valid.

Raw response:

```http
GET {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}/files/pages/A.md
Authorization: Bearer {TOKEN}
```

```http
200 OK
X-Uniseq-Remote-Version: v1

# Hello
```

JSON response:

```json
{
  "path": "pages/A.md",
  "remote_version": "v1",
  "size": 8,
  "updated_at": "2026-05-19T12:01:00Z",
  "content": [35, 32, 72, 101, 108, 108, 111, 10]
}
```

## 7. Update the File with Compare-And-Set

Request:

```http
PUT {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}/files/pages/A.md
Authorization: Bearer {TOKEN}
X-Uniseq-Base-Remote-Version: {V1}

# Hello again
```

Expect:

```json
{
  "status": "accepted",
  "remote_version": "v2",
  "updated_at": "2026-05-19T12:02:00Z"
}
```

## 8. Verify Conflict Handling

Repeat the previous `PUT`, but intentionally send the stale base version `V1` again.

Expect:

```http
409 Conflict
Content-Type: application/json
```

```json
{
  "status": "conflict",
  "current": {
    "path": "pages/A.md",
    "remote_version": "v2",
    "size": 14,
    "updated_at": "2026-05-19T12:02:00Z"
  }
}
```

## 9. Delete the File with Compare-And-Set

Request:

```http
DELETE {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}/files/pages/A.md
Authorization: Bearer {TOKEN}
X-Uniseq-Base-Remote-Version: v2
Content-Type: application/json

{
  "base_remote_version": "v2"
}
```

Expect:

```json
{
  "status": "accepted",
  "remote_version": "v3",
  "updated_at": "2026-05-19T12:03:00Z"
}
```

Returning JSON with the real new `remote_version` is strongly recommended.

## 10. Delete the Workspace

Request:

```http
DELETE {SYNC_ROOT_URL}/workspaces/{WORKSPACE_ID}
Authorization: Bearer {TOKEN}
```

Expect either:

```http
204 No Content
```

or:

```json
{
  "status": "deleted"
}
```

## Pass Criteria

Your backend is ready to try in the current Uniseq app if:

- all ten checks work
- accepted file writes and deletes return JSON with real `remote_version` values
- stale base versions reliably return `409 Conflict`
- pull responses include a usable `remote_version`
