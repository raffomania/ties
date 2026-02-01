set dotenv-load := true
set export := true

default:
    just --list

[group('Development')]
watch *args: development-cert migrate-database
    cargo bin systemfd --no-pid -s ${LISTEN} -- cargo bin cargo-watch -- cargo run start --listenfd {{args}}

[group('Development')]
run *args: development-cert migrate-database
    cargo run -- {{args}}

# Generate metadata for verifying SQL queries at compile time.
# Don't use SQLX_OFFLINE: the data it's based on is most likely out of date when this task runs.
[group('Codegen')]
generate-database-info: start-database (migrate-database "false")
    cargo bin sqlx-cli prepare -- --all-targets

[group('Codegen')]
generate-sbom:
    cargo bin cargo-cyclonedx --format json --describe binaries
    # Remove some fields that make the sbom non-reproducible.
    # https://github.com/CycloneDX/cyclonedx-rust-cargo/issues/556
    # https://github.com/CycloneDX/cyclonedx-rust-cargo/issues/514
    jq --sort-keys '.components |= sort_by(.purl) | del(.serialNumber) | del(.metadata.timestamp) | del(..|select(type == "string" and test("^path\\+file")))' ties_bin.cdx.json > ties.cdx.json
    rm ties_bin.cdx.json


[group('Database')]
start-database:
    #!/usr/bin/env bash
    set -euxo pipefail

    if podman ps --format "{{{{.Names}}" | grep -wq ties_postgres; then
        echo "Database is running."
        exit
    fi

    if ! podman inspect ties_postgres &> /dev/null; then
        podman create \
            --name ties_postgres \
            --health-cmd="pg_isready" \
            --health-startup-cmd="pg_isready" --health-startup-interval=2s \
            -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=${DATABASE_NAME} \
            -p ${DATABASE_PORT}:5432 docker.io/postgres:15 \
            postgres
    fi

    podman start ties_postgres

    podman wait --condition=healthy ties_postgres

[group('OIDC')]
start-rauthy:
    #!/usr/bin/env bash
    set -euxo pipefail

    # TODO: extract helpers for repetitive podman tasks.
    if podman ps --format "{{{{.Names}}" | grep -wq ties_rauthy; then
        echo "Rauthy is running."
        exit
    fi

    if ! podman inspect ties_rauthy &> /dev/null; then
        podman create \
            --replace --name ties_rauthy \
            --pull=missing \
            -e COOKIE_MODE=danger-insecure \
            -e PUB_URL=localhost:${RAUTHY_PORT} \
            -e LOG_LEVEL=info \
            -e LOCAL_TEST=true \
            -e BOOTSTRAP_ADMIN_EMAIL=admin@rauthy.localhost \
            -e BOOTSTRAP_ADMIN_PASSWORD_PLAIN=test \
            -e DATABASE_URL=sqlite:data/rauthy.db \
            -p ${RAUTHY_PORT}:8080 \
            ghcr.io/sebadob/rauthy:0.29.4
    fi

    podman start ties_rauthy

[group('OIDC')]
stop-rauthy:
    podman stop ties_rauthy

[group('OIDC')]
wipe-rauthy: stop-rauthy
    podman rm ties_rauthy

[group('Database')]
stop-database:
    podman stop --ignore ties_postgres

# Delete the whole development database and create a new, empty one.
# SQLX_OFFLINE=false: when migrating an empty db, checking queries against
# it would fail during compilation
[group('Database')]
wipe-database: stop-database && (migrate-database "false")
    podman rm --ignore ties_postgres

# Migrate the database.
# Allows overriding the SQLX_OFFLINE environment variable using a justfile parameter.
[group('Database')]
migrate-database sqlx_offline=env("SQLX_OFFLINE"): start-database
    SQLX_OFFLINE={{sqlx_offline}} cargo run -- db migrate

[group('Database')]
exec-database-cli: start-database
    podman exec -ti -u postgres ties_postgres psql ${DATABASE_NAME}

[group('Testing')]
start-test-database:
    #!/usr/bin/env bash
    set -euxo pipefail

    if podman ps --format "{{{{.Names}}" | grep -wq ties_postgres_test; then
        echo "Test database is running."
        exit
    fi

    if ! podman inspect ties_postgres_test &> /dev/null; then
        podman create \
            --replace --name ties_postgres_test --image-volume tmpfs \
            --health-cmd pg_isready --health-interval 10s \
            --health-startup-cmd="pg_isready" --health-startup-interval=2s \
            -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=${DATABASE_NAME_TEST} \
            -p ${DATABASE_PORT_TEST}:5432 --rm docker.io/postgres:16 \
            postgres \
            -c fsync=off \
            -c synchronous_commit=off \
            -c full_page_writes=off \
            -c autovacuum=off
    fi

    podman start ties_postgres_test

    podman wait --condition=healthy ties_postgres_test

[group('Testing')]
test *args: start-test-database generate-database-info
    # SQLX_OFFLINE: Without it, `cargo test` would compile against the test db
    # which is always empty and only migrated inside the tests themselves.
    DATABASE_URL=${DATABASE_URL_TEST} SQLX_OFFLINE=true cargo test {{args}}

[group('Development')]
development-cert: (ensure-command "mkcert")
    mkdir -p development_cert
    test -f development_cert/localhost.crt || mkcert -cert-file development_cert/localhost.crt -key-file development_cert/localhost.key localhost ties.localhost 127.0.0.1 ::1

[group('Development')]
insert-demo-data: migrate-database
    cargo run -- insert-demo-data

# Run most of the CI checks locally. Convenient to check for errors before pushing.
[group('Development')]
ci-dev: migrate-database start-test-database && generate-sbom generate-database-info
    #!/usr/bin/env bash
    set -euxo pipefail

    export RUSTFLAGS="-D warnings"
    # Prevent full recompilations in the normal dev setup which has different rustflags
    export CARGO_TARGET_DIR="target_ci"

    cargo build --release

    just lint
    just format
    just test
    just check-zizmor

# Build a production-ready OCI container using podman. Used for local testing & debugging.
[group('Testing')]
build-podman-container target="release":
    #!/bin/sh
    [[ "{{target}}" == "debug" ]] && cargo_flag="" || cargo_flag="--{{target}}"
    cargo build $cargo_flag

    podman build --format docker --platform linux/amd64 --manifest ties -f Containerfile target/{{target}}

[group('Code Quality')]
lint *args: reuse-lint
    cargo clippy {{args}} -- -D warnings

[group('Code Quality')]
lint-fix *args: reuse-lint
    cargo clippy --fix {{args}}
    cargo fix --allow-staged --all-targets

[group('Code Quality')]
reuse-lint: (ensure-command "reuse")
    reuse --root . lint

[group('Code Quality')]
format:
    cargo +nightly fmt --all

# Run the pre-commit hook script.
[group('Code Quality')]
pre-commit:
    ./pre-commit.sh

[group('Setup')]
install-git-hooks:
    ln -srf pre-commit.sh .git/hooks/pre-commit

# Run extended checks that are not part of the normal CI pipeline.
[group('Code Quality')]
check-extended: verify-msrv build-podman-container check-example-docker-compose check-zizmor

[group('Code Quality')]
check-example-docker-compose:
    #!/usr/bin/env bash

    docker-compose -f doc/docker-compose.yml -f doc/.docker-compose.test.yml up -d

    curl --retry 3 --retry-all-errors http://localhost:3000

    docker-compose -f doc/docker-compose.yml down

# Check GitHub Actions workflows for security problems.
[group('Code Quality')]
check-zizmor:
    RUST_LOG=INFO zizmor --strict-collection --pedantic .

[group('Code Quality')]
verify-msrv: (ensure-command "cargo-msrv")
    cargo msrv verify

# Diagnose potential problems in the development environment.
[group('Setup')]
doctor:
    #!/usr/bin/env bash

    just ensure-command "podman"
    just ensure-command "cargo"
    just ensure-command "cargo-bin"
    just ensure-command "mkcert"

    [[ -f .env ]] || echo ".env file is missing. Please copy .env.example and adjust it for your environment."

[private]
ensure-command +command:
    #!/usr/bin/env bash
    set -euo pipefail

    read -r -a commands <<< "{{ command }}"

    for cmd in "${commands[@]}"; do
        if ! command -v "$cmd" > /dev/null 2>&1 ; then
            printf "Couldn't find required executable '%s'\n" "$cmd" >&2
            exit 1
        fi
    done
