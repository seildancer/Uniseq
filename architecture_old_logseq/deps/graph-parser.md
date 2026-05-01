# `deps/graph-parser`

This package parses a graph directory into a working database.

## Role

- read graph files
- parse blocks and properties
- build the initial DB model
- support command-line and frontend use

## Design choice

The parser is separated so graph parsing can run independently of the UI app.

