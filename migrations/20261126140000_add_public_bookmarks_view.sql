create view public_bookmarks as
select bookmarks.*
from bookmarks
where exists (
    select 1
    from lists
    inner join links on lists.id = links.src_list_id
    where links.dest_bookmark_id = bookmarks.id
        and not lists.private
        and lists.ap_user_id = bookmarks.ap_user_id
);
