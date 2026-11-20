alter table ap_users
    add column created_at timestamp with time zone
        default current_timestamp
        not null;
