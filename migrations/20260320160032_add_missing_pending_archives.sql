insert into archives
(bookmark_id, status)
select bookmarks.id, 'Pending'
from bookmarks
where not exists (
    select id from archives
    where archives.bookmark_id = bookmarks.id
)
