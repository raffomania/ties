-- For looking up the links of a list: the feed cache signal query, which runs
-- on every feed request, and the list page queries.
create index on links(src_list_id);
