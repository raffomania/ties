use htmf::prelude_inline::*;

use crate::{
    db::{self, AppTx},
    form_errors::FormErrors,
    forms,
    response_error::{ResponseError, ResponseResult},
    views::layout,
};

pub struct Data {
    pub layout: layout::Template,
    pub invited_by_username: String,
    pub form_input: Option<forms::users::CreateUser>,
    pub errors: FormErrors,
    pub invite: db::Invite,
}

impl Data {
    pub async fn from_db(tx: &mut AppTx, token: &str) -> ResponseResult<Self> {
        let layout = layout::Template::from_db(tx, None).await?;

        let invite = db::invites::by_token(tx, token).await?;

        let Some(invite) = invite else {
            return Err(ResponseError::InvalidForm(expired(&layout).into()));
        };

        let invited_by_username = db::users::by_id(tx, invite.invited_by).await?.username;

        Ok(Self {
            layout,
            invited_by_username,
            form_input: None,
            errors: FormErrors::default(),
            invite,
        })
    }
}

pub fn expired(layout: &layout::Template) -> Element {
    layout::layout(
        p(
            [class("flex p-2 items-center justify-center h-full")],
            "This invite has expired. Please request a new one.",
        ),
        layout,
    )
}

pub fn view(data: &Data) -> Element {
    layout::layout(
        [section(
            [class(
                "flex mx-auto flex-col p-2 justify-center max-w-lg h-full",
            )],
            [intro(&data.invited_by_username), signup_form(data)],
        )],
        &data.layout,
    )
}

fn intro(invited_by_username: &str) -> Element {
    fragment([
        img([
            src("/assets/logo_icon_only.svg"),
            class("w-24 max-w-full self-center my-4"),
        ]),
        h1([class("text-center font-bold text-xl mb-2")], "Welcome!"),
        p(
            (),
            [
                a(
                    [
                        href(format!("/user/{invited_by_username}")),
                        class("underline"),
                    ],
                    invited_by_username,
                ),
                span((), " has invited you to join their "),
                a(
                    class("font-bold text-purple-400 dark:text-purple-300"),
                    "ties",
                ),
                span(
                    [],
                    " server. Here, you can save, organise and share links to websites you like.",
                ),
            ],
        ),
        p(
            class("mt-4"),
            "To get started, please choose a username and password.",
        ),
    ])
}

fn signup_form(
    Data {
        form_input, errors, ..
    }: &Data,
) -> Element {
    form(
        [
            method("POST"),
            class(
                "border shadow-sm border-neutral-300 dark:border-neutral-700 rounded p-2 sm:p-6 \
                 mt-6",
            ),
        ],
        [
            label(
                [class("block")],
                [
                    span((), "Username"),
                    span(
                        class("text-neutral-500 dark:text-neutral-400"),
                        " (Only letters, numbers and underscores)",
                    ),
                    input([
                        value(form_input.as_ref().map_or("", |i| i.username.as_str())),
                        class(
                            "w-full rounded py-1.5 px-3 mt-2 bg-white dark:bg-neutral-900 block \
                             border border-neutral-300 dark:border-neutral-700",
                        ),
                        name("username"),
                        required(""),
                        type_("text"),
                    ]),
                ],
            ),
            errors.view("username"),
            label(
                [class("mt-4 block")],
                [
                    span((), "Password"),
                    span(
                        class("text-neutral-500 dark:text-neutral-400"),
                        " (10 characters or more)",
                    ),
                    input([
                        class(
                            "w-full rounded py-1.5 px-3 mt-2 bg-white dark:bg-neutral-900 block \
                             border border-neutral-300 dark:border-neutral-700",
                        ),
                        name("password"),
                        required(""),
                        type_("password"),
                    ]),
                ],
            ),
            errors.view("password"),
            p(
                class("mt-4"),
                "Please use a password manager to remember your password. At the moment, ties \
                 does not have a way to reset it.",
            ),
            button(
                [
                    class(
                        "rounded w-full bg-neutral-600 dark:bg-neutral-200 text-white \
                         dark:text-neutral-800 px-4 py-2 mt-4 font-bold hover:bg-neutral-700 \
                         dark:hover:bg-neutral-100",
                    ),
                    type_("submit"),
                ],
                "Create account",
            ),
        ],
    )
}
