create type archive_status as enum ('Success', 'Error');
create table archives (
    id uuid primary key
        default gen_random_uuid()
        not null,

    bookmark_id uuid
        references bookmarks(id)
        not null,

    created_at timestamp with time zone
        default current_timestamp
        not null,
    status archive_status
        not null,
    error_description varchar(5000)
        default null,
    extracted_html
        -- With a limit of 5MB per page and min. 1 byte per char we can take a maximum of 5 million chars
        varchar(5000000)
        not null,

    -- only one archive per bookmark for now
    unique (bookmark_id)
);
