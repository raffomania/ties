alter table lists add search tsvector generated always as (
    setweight(to_tsvector('english', title), 'A')
) stored;

create index on lists using gin(search);
