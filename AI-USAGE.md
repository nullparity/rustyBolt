This file states where this project uses AI. It names no vendor. The maintainer is responsible for every line of code and text in the repository.

A coding assistant helped write and edit the source code, the tests, the comments, the build scripts, the README, `CONTRACT.md` and this file. The application itself uses no AI. It makes no call to an inference API. It sends user data only to the game vendor, the client download hosts and the Java runtime that the user picks.

The maintainer reads every change before it enters `main`. Continuous integration runs `cargo fmt`, `cargo clippy` and `cargo test` on macOS, Linux and Windows. A local lint rejects comments and commit messages that narrate the edit instead of the code.

Development tools can send source text to an external model. The repository holds no secret, so no secret leaves the machine. The application stores the session token, the character list and the config in the user directories that `rustybolt paths` prints. No part of that data reaches an AI service.

If you use AI to make a change, read the output before you open a pull request. State the change in your own words. Do not add an attribution line for the tool in a commit message or a pull request body.
