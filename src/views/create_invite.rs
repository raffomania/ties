use htmf::prelude::*;
use url::Url;

use crate::{
    db,
    response_error::ResponseResult,
    views::{common, layout},
};

pub struct Created {
    pub layout: layout::Template,
    pub base_url: Url,
    pub token: String,
}

pub fn created(
    Created {
        layout,
        token,
        base_url,
    }: Created,
) -> ResponseResult<Element> {
    let invite_url = base_url.join("invites/")?.join(&token)?.to_string();

    Ok(layout::layout(
        [
            layout::upper_border(),
            section(
                [class(
                    "flex flex-col items-stretch mx-auto my-8 p-2 gap-2 max-w-xl",
                )],
                [
                    h1(
                        [class("my-4 font-bold text-xl w-full max-w-lg")],
                        "Invite someone to ties",
                    ),
                    p(
                        [class("basis-full")],
                        "Send this link to the user you'd like to invite:",
                    ),
                    pre(
                        class(
                            "py-4 px-4 border border-neutral-200 dark:border-neutral-600 rounded \
                             bg-neutral-100 dark:bg-neutral-900 overflow-auto max-w-full",
                        ),
                        invite_url.clone(),
                    ),
                    common::clipboard_button(
                        class(
                            "js-only px-4 py-2 bg-neutral-200 hover:bg-neutral-300 \
                             dark:bg-neutral-700 dark:hover:bg-neutral-600 rounded",
                        ),
                        &invite_url,
                        "Copy to clipboard",
                    ),
                    p(
                        [],
                        format!(
                            "The link will be valid for the next {} hours.",
                            db::invites::VALID_DURATION.whole_hours()
                        ),
                    ),
                ],
            ),
        ],
        &layout,
    ))
}
