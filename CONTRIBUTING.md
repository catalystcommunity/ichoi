# Contributing to Ichoi

Thanks for your interest. These are the working conventions for this repository; they
apply regardless of what task is being done. Read [`docs/DESIGN.md`](docs/DESIGN.md) first
— it is the source of truth for architecture and the reasoning behind every decision.

## Licensing and contributions

Ichoi is **Apache-2.0** (see [`LICENSE`](LICENSE)). There is **no CLA**. By submitting a
contribution you agree it is licensed under Apache-2.0, per section 5 of the license
("Submission of Contributions") — the inbound license is the outbound license, nothing
more to sign.

Keep the binary clean: **nothing copyleft or patent-encumbered goes inside the Ichoi
binary** (DESIGN §13). The bundled ffmpeg is LGPL and stays a *separate, subprocessed
program* — never linked (see [`docs/ffmpeg.md`](docs/ffmpeg.md)). If a dependency you want
to add is GPL, LGPL-as-a-linked-crate, or patent-encumbered, it does not go in.

## Commits: conventional commits

Every commit message must follow [Conventional Commits](https://www.conventionalcommits.org/).
CI (`.reactorcide/jobs/conventional-commits.yaml`) rejects any PR whose commits don't, and
`semver-tags` derives the release version from them.

```
type(scope)?: short description
```

Valid types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`,
`chore`, `revert`. A `!` (or a `BREAKING CHANGE:` footer) marks a breaking change and bumps
the major version.

## Commit discipline — agents and humans

- **Do not commit or push without being asked.** Agents especially: make the change, leave
  it in the working tree, and let the human commit. Don't create branches, commits, tags,
  or PRs unless the task explicitly says to.
- Keep changes focused. Don't fix unrelated things in the same logical change unless
  they're blocking; if you must, note it so it can be split.
- **All tests must pass at all times.** A failing test is a blocking issue whether or not
  it relates to your change.

## Generated code is never hand-edited

The CSIL schema in `schema/` is the source of truth for types and service interfaces.
Everything under `generated/` is emitted by `csilgen` and **must never be edited by hand**.

- Regenerate with `./tools.sh gen` (or `gen-server` for just the Rust server bindings).
  Generated files are checked in but must be reproducible — CI
  (`.reactorcide/jobs/csil.yaml`) validates the schema and fails if the checked-in output
  is stale.
- If the generator lacks a capability you need, or emits something wrong, **do not paper
  over it here.** File a request in the csilgen repo's inbox —
  `~/repos/catalystcommunity/csilgen/docs/csilgen-requests/` — one markdown file per
  request with a `Status:` line, describing the problem from our (consumer) perspective:
  the CSIL that triggers it, what's wrong or missing, and how to verify the fix. That is
  where csilgen picks up work; there is **no** `docs/csilgen-requests/` in this repo.

## Before you consider work done

Run the full local gate — it mirrors CI:

```sh
./tools.sh check     # csil-validate + cargo fmt + clippy (-D warnings) + SQLite tests
```

Then review your own diff antagonistically: unnecessary complexity, missing error handling
at system boundaries (user input, network, external processes), security issues, coupling
that violates the architecture boundaries in DESIGN, and tests that don't actually assert
anything. Would you give it a 95% on review? If not, fix it before handing it off.

## Testing conventions

- **Real database, rolled back.** Integration tests run against a real SQLite database
  inside a transaction that rolls back on drop (DESIGN §12). No mocks for the data layer.
- **Transaction isolation is non-negotiable** — each test gets its own transaction, so the
  suite parallelizes safely and tests never see each other's data.
- **Don't couple tests to models.** Specify only the fields a test cares about; factories
  fill defaults for the rest. When a model gains a required column, only tests exercising
  that column change.
- SQLite is the only backend (DESIGN §2); switch with `TEST_DATABASE_BACKEND=sqlite`
  (what `./tools.sh test` sets).
- **Test media**: a few small public-domain files, committed if small, **never** bundled
  into the binary or a container image.
