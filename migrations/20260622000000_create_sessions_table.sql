create table sessions (
    session_key text primary key,
    contents jsonb not null,
    expires_at timestamptz not null
);

drop schema if exists tower_sessions cascade;
