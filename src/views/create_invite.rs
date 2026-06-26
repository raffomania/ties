use htmf::prelude_inline::*;
use url::Url;

use crate::{db, response_error::ResponseResult, views::layout};

pub struct Data {
    pub layout: layout::Template,
    pub base_url: Url,
    pub token: String,
}

pub fn view(
    Data {
        layout,
        token,
        base_url,
    }: Data,
) -> ResponseResult<Element> {
    let invite_url = base_url.join("invites/")?.join(&token)?.to_string();

    Ok(layout::layout(
        [section(
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
                        "py-4 px-4 border border-neutral-600 rounded bg-neutral-900 overflow-auto \
                         max-w-full",
                    ),
                    invite_url.clone(),
                ),
                clipboard_button(invite_url),
                p(
                    [],
                    format!(
                        "The link will be valid for the next {} hours.",
                        db::invites::VALID_DURATION.whole_hours()
                    ),
                ),
            ],
        )],
        &layout,
    ))
}

fn clipboard_button(invite_url: &str) -> Element {
    let description = "Copy to clipboard";
    button(
        [
            class("js-only px-4 py-2 bg-neutral-700 hover:bg-neutral-600 rounded"),
            attr(
                "onclick",
                format!(
                    "navigator.clipboard.writeText('{invite_url}')
                    .then(() => {{
                        event.target.textContent='Copied!';
                        setTimeout(() => event.target.textContent = '{description}', 4000)
                    }})",
                ),
            ),
        ],
        description,
    )
}
