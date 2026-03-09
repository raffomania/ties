use anyhow::Result;
use garde::Validate;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

use crate::federation::{self, person::Person};

#[derive(Validate)]
pub struct CreateApUser {
    #[garde(skip)]
    pub id: Uuid,
    #[garde(adapt(crate::forms::validate_url), length(max = 255))]
    pub ap_id: Url,
    #[garde(length(max = 50))]
    pub username: String,
    #[garde(adapt(crate::forms::validate_url), length(max = 255))]
    pub inbox_url: Url,
    #[garde(length(max = 10_000))]
    pub public_key: String,
    #[garde(length(max = 10_000))]
    pub private_key: Option<String>,
    #[garde(skip)]
    pub last_refreshed_at: OffsetDateTime,
    #[garde(length(max = 100))]
    pub display_name: Option<String>,
    #[garde(length(max = 5_000))]
    pub bio: Option<String>,
}

impl CreateApUser {
    /// Create a new local activitypub user with a private key
    pub fn new_local(base_url: &Url, username: String) -> Result<Self> {
        let id = Uuid::new_v4();
        let ap_id = base_url.join("/ap/user/")?.join(&id.to_string())?;

        let inbox_url = base_url.join("/ap/inbox/")?.join(&id.to_string())?;

        let ap_keypair = federation::signing::generate_keypair()?;
        let create = CreateApUser {
            id,
            ap_id,
            username,
            inbox_url,
            public_key: ap_keypair.public_key,
            private_key: Some(ap_keypair.private_key),
            last_refreshed_at: OffsetDateTime::now_utc(),
            display_name: None,
            bio: None,
        };

        create.validate()?;

        Ok(create)
    }

    /// Create a new activitypub user from a different instance - without a
    /// private key
    pub fn new_remote(json: Person) -> Result<Self> {
        let create_user = CreateApUser {
            id: Uuid::new_v4(),
            ap_id: json.id.into_inner(),
            username: json.preferred_username,
            inbox_url: json.inbox,
            public_key: json.public_key.public_key_pem,
            private_key: None,
            last_refreshed_at: OffsetDateTime::now_utc(),
            display_name: json.name,
            bio: json.summary,
        };

        create_user.validate()?;

        Ok(create_user)
    }
}

// Currently only used in insert-demo-data script
#[allow(dead_code)]
#[derive(Validate)]
pub struct UpdateApUser {
    #[garde(length(max = 100))]
    pub display_name: Option<String>,
    #[garde(length(max = 1_000))]
    pub bio: Option<String>,
}
