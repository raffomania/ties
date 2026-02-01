#!/bin/bash

just start-database
just migrate-database

if ! cargo bin sqlx-cli prepare --check -- --all-targets; then
    just generate-database-info
    exit 1
fi
