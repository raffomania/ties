-- Created for the user feed cache signal, but probably used in other places as well.
create index on bookmarks(ap_user_id);
create index on lists(ap_user_id);
create index on links(user_id);
create index on users(ap_user_id);
