alter table bookmarks add search tsvector default null;

create index on bookmarks using gin(search);
