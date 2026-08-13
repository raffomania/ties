-- For getting newest bookmarks
CREATE INDEX idx_bookmarks_created_at ON bookmarks (created_at);

-- for filtering links by source list
CREATE INDEX idx_links_src_list_id ON links (src_list_id);

-- for joining bookmarks to followed users
CREATE INDEX idx_bookmarks_ap_user_id ON bookmarks (ap_user_id);

-- for filtering lists by private flag
CREATE INDEX idx_lists_private ON lists (private) WHERE NOT private;
