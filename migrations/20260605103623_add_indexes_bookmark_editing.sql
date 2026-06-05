-- For updating search index
create index on archives(bookmark_id);
-- For finding backlinks
create index on links(dest_bookmark_id);
