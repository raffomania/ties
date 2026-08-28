create table list_bookmarked_user_follows (
    id uuid primary key
        default gen_random_uuid()
        not null,
    list_id uuid
        references lists(id)
        not null,
    bookmark_id uuid
        references bookmarks(id)
        not null,
    followed_ap_user_id uuid
        references ap_users(id)
        not null,
    follow_id uuid
        references follows(id)
        not null
);
