# Filesystem Abstraction

The app is built around a filesystem protocol rather than one concrete backend.

## Supported environments

- browser storage
- Electron desktop
- Capacitor mobile
- Node/CLI tools

## Protocol responsibilities

- create directories
- read and write files
- rename and copy paths
- delete paths
- stat files
- enumerate directories
- watch and unwatch folders

This layer isolates platform details from the rest of the app.

