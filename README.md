# vx
> [!WARNING]
>
>  Urgent info that needs immediate user attention to avoid problems.
> ----------------------------
> vx is actively under development and NOT yet stable.
> Expect breaking CLI changes, rough edges, and bugs.
>
> If you try it:
> - read output carefully
> - pin versions if you script against it
> - report issues with command + output + Void version

---

vx is a front door for Void Linux packages.

It unifies the workflows people actually use every day:
- xbps-install / xbps-query (repo packages)
- void-packages / xbps-src (source packages)

The goal is NOT to replace XBPS.
The goal is to make common tasks faster, clearer, and consistent.

Void already has great tools — vx focuses on flow.

---

## Why vx?

Common pain points vx smooths out:
- remembering which xbps-* command does what
- switching mental modes between repos and void-packages
- repeating confirmation flags everywhere
- tracking which source packages you actually care about

vx is intentionally:
- small
- transparent (runs the real tools)
- boring in the good way
- very “Void-ish”

No hidden magic.

---

## Install

Currently vx is built from source.

    git clone <repo>
    cd vx
    cargo build --release
    ./target/release/vx status

Optional install:

    sudo install -m755 target/release/vx /usr/local/bin/vx

---

## Configuration

vx works with sane defaults but supports overrides.

Run:

    vx status

This shows:
- config file detection
- resolved xbps tool paths
- sudo usage
- void-packages resolution (cli / env / config)
- local repo path and nonfree usage
- managed source package list

void-packages can be provided via:
- --voidpkgs /path/to/void-packages
- VX_VOIDPKGS=/path/to/void-packages
- config file

---

## Repo Workflow (XBPS)

Search repositories:

    vx search discord

Search installed packages:

    vx search --installed discord
    vx search -i discord

Show repo package info:

    vx info ripgrep

List installed files:

    vx files ripgrep

Find which package owns a path:

    vx provides /usr/bin/rg

Install packages:

    vx add ripgrep fd
    vx add -y ripgrep

Remove packages:

    vx rm ripgrep
    vx rm -y ripgrep

System upgrade:

    vx up
    vx up -y

---

## Source Workflow (void-packages / xbps-src)

Search source packages:

    vx src search discord
    vx src search --installed discord

Build:

    vx src build discord

Clean:

    vx src clean discord

Lint:

    vx src lint discord

Install from local repo:

    vx src add discord
    vx src add -y discord

Force reinstall from local repo:

    vx src add -f discord

Rebuild + reinstall (clean + build + install):

    vx src add --rebuild discord
    vx src add --rebuild -y discord

Update source packages:

    vx src up discord
    vx src up --all
    vx src up --all -y
    vx src up --all -f -y

---

## Managed Source Package

When you install a package via:

    vx src add <pkg>

vx adds it to a managed list.

That list is used by:

    vx src up --all
    vx up --all

So you only rebuild the source packages you actually care about.

---

## Safety / Guardrails

vx avoids ambiguous or destructive combinations.

Example:
- --force = reinstall from local repo
- --rebuild = clean + build + reinstall

Using both together is rejected.

vx prefers explicit intent.

---

## Philosophy / Non-Goals

### vx is NOT:
- a replacement for XBPS
- a distro layer
- an AUR helper clone
- a wrapper that hides what it’s doing

### vx IS:
- a thin, honest front door
- fast to type
- easy to remember
- aligned with Void’s design philosophy

---

## Near-term Roadmap

High-value, low-bloat goals:
- improved search formatting
- consistent installed markers for src search
- optional helpers for restricted void-packages builds (opt-in)
- cleaner status output
- man page + shell completions

---

## Contributing

Issues and PRs welcome.

For bug reports, include:
- Void version + architecture
- vx version (vx status)
- exact command run
- stdout + stderr
- how void-packages was provided (cli/env/config)

---

## License

MIT

