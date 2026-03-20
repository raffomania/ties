update bookmarks
set search =
    setweight(to_tsvector('english', bookmarks.title), 'A')
    ||
    setweight(
        to_tsvector('english',
            regexp_replace(bookmarks.url, '[\./]', ' ', 'g'))
        ||
        to_tsvector('english', bookmarks.url)
    , 'C')
where search is null;
