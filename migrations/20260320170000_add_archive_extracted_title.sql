alter table archives
    add extracted_title varchar default null;
alter table archives
    add byline varchar default null;
alter table archives
    add lang varchar default null;
alter table archives
    add site_name varchar default null;
alter table archives
    add published_time timestamp with time zone default null;
