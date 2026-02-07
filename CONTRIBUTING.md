# Contributing to ties

Welcome! We're excited that you are interested in contributing to ties. There are many ways to to so, and every small bit helps.

You can:

- Test & report bugs
- Improve the documentation
- Propose new features
- Run your own instance
- Create UI concepts for new features
- Discuss how features should work with others
- Write code
- Distribute stickers

## 📋 Guidelines

- **Do not commit LLM-generated code, and do not post LLM output in issues or discussions.** This includes AI-assisted refactoring, autocomplete, and generated tests. You can use LLMs for other tasks (e.g. reviewing your PR before submitting), but we currently don't have enough time to properly review LLM-generated code or read LLM-generated text in discussions.
- **Ask questions.** Comment on issues, start discussions on GitHub, write an email to maintainers: this will help you reach your goal faster, reduce review time for your PRs, and provide feedback on how the project can improve - everybody wins!

## 🗨️ Join the Community

The easiest way to talk to someone is using [GitHub discussions](https://github.com/raffomania/ties/discussions). You can ask questions, brainstorm feature ideas, share how you're using ties, or post cat pics!

If you have a feature request, found a bug, or want to request any other change to the software itself, please [create an issue](https://github.com/raffomania/ties/issues/new).

## 🧪 Testing

Testing is most valuable for features which haven't been released yet, so we can catch bugs before the majority of users encounters them.
You can try unreleased features out in two ways.

The first and easy one is to play around on [demo.ties.pub](https://demo.ties.pub), which is sporadically updated to the newest version of the `main` branch.

The second one is to set up your own ties instance using the `latest` container tag or by building the `main` branch from source.

Once you find a bug, please create an issue on GitHub.
Make sure to include the detailed steps you used to trigger the bug!

## 📝 Writing Docs

The developer docs live in the [doc](./doc/index.md) folder of the ties source code. Feel free to open a PR for anything that's missing or outdated!

If you're interested in contributing to ties' user manual, check out [this issue](https://github.com/raffomania/ties/issues/254).

## 🎨 UI/UX

If you would like to help with UI design or improve ties' UX, reach out to a maintainer via mail - we don't have a lot of experience here yet, but would love to have you onboard!

## 💻 Writing Code

If you have a feature in mind that you want to build, search [the issue tracker](https://github.com/raffomania/ties/issues) and [discussions](https://github.com/raffomania/ties/discussions) to see if people have discussed it before.
If there is no issue yet, please open one.
Propose a workflow from the user's perspective, and an implementation plan, to make sure the way you'll implement the feature works well with the goals of the project.
This will reduce the time needed to review and merge your PR greatly :)

Follow instructions in the [README](./README.md) to set up a development environment.
Happy hacking! Don't hesitate asking for help if you're unsure about something.

### Philosophy

- Try to avoid adding crate dependencies.
They increase the maintenance burden and the attack surface for security vulnerabilities.
That said, if there's a high-quality crate available that saves us from a big chunk of work, it makes sense to use it!
- Deploying ties cannot require any external service besides PostgreSQL. Use PostgreSQL or Rust to do the job.
- Where possible, use Rust on the server instead of JS in the browser.

## Stickers

If you'd like to receive a bunch of stickers to distribute at conferences, for family or friends - write an email to [Rafael](https://github.com/raffomania/).
