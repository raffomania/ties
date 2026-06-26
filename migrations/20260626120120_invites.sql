create table invites (
    id uuid
        primary key
        default gen_random_uuid()
        not null,
    created_at timestamp with time zone
        default current_timestamp
        not null,
    invited_by uuid
        references users(id)
        not null,
    token varchar(20)
        not null
);

alter table users
    add column invited_by uuid
        references users(id)
        default null
;
