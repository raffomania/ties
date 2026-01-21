# Writing Migrations

ties uses a custom migration system that wraps the migration runner from the sqlx crate.
This allows us to run arbitrary rust code before and after the SQL part of a migration, within the same transaction.

However, it means that the rust code in the project needs to compile successfully so you can run the migrations.
Because we use sqlx, this means that the database must match the schema the queries in the rust code expect.

When adding a new migration, follow these steps:

1. Create the migration using `sqlx migrate add`.
1. Write your migration code.
1. Migrate the database using `just migrate-database`.
1. Commit your migration to a separate commit. If you need to change the new migration later on, you can check out this commit and run `just wipe-database` to compile the project without any errors.
1. Queries conflicting with the new database schema will now cause compilation to fail. Update these to make the code compile again.
1. Run `just generate-database-info` so you can compile with `SQLX_OFFLINE=true`.
