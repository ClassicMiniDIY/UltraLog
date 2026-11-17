# Linux build: pin the glibc floor (issue #113)

Status: plan · 2026-11-17

## Problem

`ultralog-linux` 3.1.1 fails to start on Ubuntu 24.04 (and on every distro with glibc < 2.43):

```text
./ultralog-linux: /lib/x86_64-linux-gnu/libm.so.6: version `GLIBC_2.43' not found
```

## Root cause (verified)

`build-linux` in `.github/workflows/release.yml` runs on `ubuntu-latest`. Between the 3.1.0 and
3.1.1 tags, GitHub moved `ubuntu-latest` to 26.04. Commit 34553c0, which swapped
`libgdk-pixbuf2.0-dev` for `libgdk-pixbuf-2.0-dev`, was the fix for that same move. A Rust
binary links each libc symbol at the newest version the build host's glibc exports, so the
26.04 host (glibc 2.43) produced a binary that needs 2.43.

`objdump -T` on the published release assets:

| Build | Newest strong glibc symbol | Newest weak symbol | Runs on |
|---|---|---|---|
| v3.1.0 (built on 24.04) | `hypot@GLIBC_2.35` | `pidfd_spawnp@GLIBC_2.39` (weak, optional) | glibc ≥ 2.35 |
| v3.1.1 (built on 26.04) | **`atan2f@GLIBC_2.43`** | same | glibc ≥ 2.43 only |
| current `main` built locally on Ubuntu 24.04 | `hypot@GLIBC_2.35` | same | glibc ≥ 2.35 |

On this session's Ubuntu 24.04 container (glibc 2.39), `ldd` on the 3.1.1 asset fails with the
reporter's exact error. On the local 24.04 build of `main` it reports nothing missing. The
binary's only `NEEDED` libraries are `libc`, `libm` and `libgcc_s`. X11, Wayland, GL and
xkbcommon are loaded with `dlopen`, so glibc is the only floor that matters.

The weak `GLIBC_2.39` references are Rust std's optional `pidfd` support. A missing weak version
does not stop the program from loading, so they do not raise the floor.

## Plan

### Phase 1: hotfix (shipped in 3.2.0, together with #114)

1. **Pin the runner.** In `release.yml` `build-linux`, change `runs-on: ubuntu-latest` to
   `runs-on: ubuntu-24.04`, with a comment explaining the glibc floor and pointing to this doc.
   The resulting floor is glibc 2.35, exactly what 3.1.0 shipped: Ubuntu 22.04+, Debian 12+,
   Mint 21+, Fedora 36+. The macOS and Windows jobs are unaffected. Leave `ci.yml` on
   `ubuntu-latest` (it only lints and tests).
2. **Add a glibc gate**, so a runner change can never ship this again silently. Add a step after
   *Build release binary* that fails the job if any strong (non-weak) symbol needs a version
   above the floor:

   ```bash
   MAX=2.35
   NEED=$(objdump -T target/x86_64-unknown-linux-gnu/release/ultralog \
     | awk '$2 != "w" && match($0, /GLIBC_[0-9.]+/) { print substr($0, RSTART+6, RLENGTH-6) }' \
     | sort -V | tail -1)
   echo "Highest required glibc: $NEED (floor $MAX)"
   if [ "$(printf '%s\n%s\n' "$NEED" "$MAX" | sort -V | tail -1)" != "$MAX" ]; then
     echo "::error::ultralog-linux needs glibc $NEED, above the supported floor $MAX"
     objdump -T target/x86_64-unknown-linux-gnu/release/ultralog | grep -E "GLIBC_($NEED)\b"
     exit 1
   fi
   ```

   Keep `MAX` in one place (a job-level `env: GLIBC_FLOOR: "2.35"`) so a deliberate raise is a
   one-line, reviewed change.
3. **Optional smoke test** in the same job:
   `docker run --rm -v "$PWD/target/x86_64-unknown-linux-gnu/release/ultralog:/u:ro" ubuntu:22.04 sh -c '! ldd /u | grep "not found"'`.
   This exercises the real loader on the oldest supported distro. It catches a missing *library*
   as well as a missing symbol version, for example if a future dependency stops using `dlopen`.
4. **Docs.** Add a short "Linux glibc floor" paragraph to the Auto-Update Archive Contract
   section of `CLAUDE.md` (runner pinned on purpose; the gate; how to raise the floor). Add
   "Requires glibc 2.35+ (Ubuntu 22.04 or newer)" to the Linux install instructions in the wiki
   (`Installation` / `User-Guide`) and to the release-notes template in `release.yml`.
5. **Release notes for 3.2.0.** Linux users who downloaded 3.1.1 on a distro older than 26.04
   never got it to start, so no updater ran. They install 3.2.0 by hand. Users still on 3.1.0
   install by hand anyway (the old Linux updater cannot find the binary; see the contract).
   Users on 26.04 who run 3.1.1 auto-update normally.
6. **Close #113** (closed by the merge of #115; link the 3.2.0 release on it once published).

Verification before tagging: run the workflow with `workflow_dispatch` (beta suffix), download
the artifact and confirm `objdump -T … | grep GLIBC_2.4` prints nothing.

### Phase 2: decouple from runner images (optional, later)

The `ubuntu-24.04` image will be retired around 2029, and pinning only postpones the problem.
The durable fix is to build against an explicit glibc target with
[`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild):

```bash
cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.31
```

- This lowers the floor to glibc 2.31 (Ubuntu 20.04, Debian 11) and makes it independent of the
  runner OS.
- Risk: the C/asm build scripts (`ring`, `zstd-sys`, `unsafe-libyaml-norway`) must build under
  `zig cc`. Not verified in this session (installing zig here was blocked). The Phase 1 gate
  catches any mistake.
- Alternative with the same effect: run `build-linux` inside an old-glibc container
  (`container: debian:bullseye` plus rustup). That is heavier to maintain than zigbuild.
- Do this when a user asks for an older distro, or when GitHub announces the 24.04 image
  deprecation. Lower `GLIBC_FLOOR` in the same change.

### Optional hardening: updater self-check

Since 3.1.1 the Linux updater really installs and relaunches. A future build that cannot load
would replace a working binary with one that does not start. Phase 1's gate makes that
unlikely. Belt and braces: in `install_linux`'s script (`src/updater.rs`), run
`"$NEW" --ultralog-selftest` before the `mv`, and skip the swap if it fails. The new flag would
exit 0 right at the top of `main`, before eframe starts. A loader error exits non-zero before
`main` runs. Add a test to `updater::tests` beside the existing fake-app script test.

## Files

- `.github/workflows/release.yml`: runner pin, gate step, optional smoke test, release-notes line
- `CLAUDE.md`: glibc floor paragraph
- wiki: Linux requirements line
- (optional) `src/updater.rs`, `src/main.rs`: self-check flag
