#!/bin/sh
set -e

SEMVER_TAGS_VERSION="v0.4.0"
GHCLI_VERSION="2.63.2"

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=release-lib.sh
. "${SCRIPT_DIR}/release-lib.sh"

# PWD is set by the caller (release.yaml) to REACTORCIDE_CODE_DIR (the repo root).

# -------------------------------------------------------------------
# Version targets (multiple-directory strategy)
# -------------------------------------------------------------------
# Each entry is a top-level directory that is independently versioned. semver-tags
# analyzes only the conventional commits that touched a target's directory and tags
# it as "<target>/vX.Y.Z" (one package per directory). Each target keeps its own
# version/VERSION.txt, and a push to that file triggers the target's build/deploy job.
#
# To add a target later (e.g. a mobile app): create `mobile/` with its source and a
# `mobile/version/VERSION.txt`, append it here, add an artifact builder in
# build_target_artifacts() below, and add a `mobile-build-and-deploy` job whose
# `paths` watches `mobile/version/VERSION.txt`. Nothing else changes.
TARGETS="server"

# -------------------------------------------------------------------
# 1. Install semver-tags
# -------------------------------------------------------------------
echo "=== Installing semver-tags ${SEMVER_TAGS_VERSION} ==="
wget -q "https://github.com/catalystcommunity/semver-tags/releases/download/${SEMVER_TAGS_VERSION}/semver-tags.tar.gz" \
  -O /tmp/semver-tags.tar.gz
tar -xzf /tmp/semver-tags.tar.gz -C /tmp
chmod +x /tmp/semver-tags
export PATH="/tmp:$PATH"

# NOTE: the caller (release.yaml) has already put us on the real main tip with
# full history + tags before invoking this script, so semver-tags sees every
# release and the version-bump commit below fast-forwards main.

# -------------------------------------------------------------------
# 2. Determine per-target version bumps from conventional commits
# -------------------------------------------------------------------
# One --directories flag per target. semver-tags emits its outputs as comma-joined
# lists in the SAME order as the --directories flags, so we split them in lockstep.
DIR_ARGS=""
for t in ${TARGETS}; do
  DIR_ARGS="${DIR_ARGS} --directories ${t}"
done

echo "=== Running semver-tags (${TARGETS}) ==="
# shellcheck disable=SC2086
semver-tags run ${DIR_ARGS} --output_json > /tmp/semver-output.txt 2>&1
OUTPUT=$(tail -1 /tmp/semver-output.txt)
echo "Output: ${OUTPUT}"

PUB_LIST=$(echo "${OUTPUT}" | grep -o '"New_release_published":"[^"]*"' | cut -d'"' -f4)
VER_LIST=$(echo "${OUTPUT}" | grep -o '"New_release_version":"[^"]*"' | cut -d'"' -f4)
TAG_LIST=$(echo "${OUTPUT}" | grep -o '"New_release_git_tag":"[^"]*"' | cut -d'"' -f4)

# Record the PUBLISHED targets as "target version tag" lines. semver-tags has already
# created and pushed each target's git tag by this point; here we decide which
# version files to stamp and which GitHub releases to cut.
: > /tmp/published-targets.txt
i=1
for t in ${TARGETS}; do
  p=$(echo "${PUB_LIST}" | cut -d, -f${i})
  v=$(echo "${VER_LIST}" | cut -d, -f${i})
  tag=$(echo "${TAG_LIST}" | cut -d, -f${i})
  if [ "${p}" = "true" ]; then
    echo "${t} ${v} ${tag}" >> /tmp/published-targets.txt
    echo "=== New release: ${t} -> ${tag} (${v}) ==="
  else
    echo "=== ${t}: no new release ==="
  fi
  i=$((i + 1))
done

# Install gh before deciding there is nothing to do: a previous attempt may have
# pushed its tag and then failed before creating the GitHub Release. semver-tags
# reports no new release on that retry, so we explicitly recover that partial state.
if [ "${SKIP_GITHUB:-false}" != "true" ]; then
  echo "=== Installing gh CLI ${GHCLI_VERSION} ==="
  wget -q "https://github.com/cli/cli/releases/download/v${GHCLI_VERSION}/gh_${GHCLI_VERSION}_linux_amd64.tar.gz" -O /tmp/gh.tar.gz
  tar -xzf /tmp/gh.tar.gz -C /tmp
  export PATH="/tmp/gh_${GHCLI_VERSION}_linux_amd64/bin:$PATH"

  recover_unreleased_targets "${TARGETS}" /tmp/published-targets.txt "${REACTORCIDE_REPO}"
fi

if [ ! -s /tmp/published-targets.txt ]; then
  echo "No new or incomplete release found for any target."
  exit 0
fi

# The version files this release stamps + a human summary, kept in lockstep with the
# published set so the edit, the git-add, and the commit message all agree.
VERSION_FILES=$(while read -r t v tag; do printf '%s/version/VERSION.txt ' "${t}"; done < /tmp/published-targets.txt)
SUMMARY=$(while read -r t v tag; do printf '%s %s, ' "${t}" "${v}"; done < /tmp/published-targets.txt | sed 's/, $//')

# Stamp each published target's version file. Deterministic and idempotent, so it can
# be re-applied after re-basing onto a newer main during the push retry.
apply_version_files() {
  while read -r t v tag; do
    echo "${v}" > "${t}/version/VERSION.txt"
  done < /tmp/published-targets.txt
}

# -------------------------------------------------------------------
# 3. Update versioned files and push the bump to main
# -------------------------------------------------------------------
echo "=== Updating versioned files (${SUMMARY}) ==="
apply_version_files

# SKIP_GITHUB=true skips push and release-create; on-disk file edits and the build still run.
if [ "${SKIP_GITHUB:-false}" = "true" ]; then
  echo "=== SKIP_GITHUB=true: skipping version-bump commit and push ==="
else
  git config user.name "Catalyst Community (automation)"
  git config user.email "automation@catalystcommunity.dev"
  git remote set-url origin "https://x-access-token:${GITHUB_PAT}@github.com/${REACTORCIDE_REPO}.git"
  # shellcheck disable=SC2086
  git add ${VERSION_FILES}
  git commit -m "ci: bump versions (${SUMMARY})" || echo "No version changes to commit"

  # Push the bump to main. We synced onto main's tip in release.yaml, so the first
  # push normally fast-forwards. If a CONCURRENT merge advances main between our
  # sync and this push, the push is non-fast-forward — re-base our bump onto the
  # fresh main and retry. Failure after every attempt is FATAL: we must never
  # `gh release create` for a commit that isn't on main.
  push_attempts=5
  n=0
  while ! git push origin HEAD:main; do
    n=$((n + 1))
    if [ "$n" -ge "$push_attempts" ]; then
      echo "ERROR: could not push the version bump to main after ${push_attempts} attempts — aborting to avoid an orphan release/tag."
      exit 1
    fi
    echo "=== main advanced; re-basing the bump onto the latest main (attempt ${n}/${push_attempts}) ==="
    git fetch --tags --prune --force origin "+refs/heads/main:refs/remotes/origin/main"
    git reset --hard origin/main
    apply_version_files
    # shellcheck disable=SC2086
    git add ${VERSION_FILES}
    if git diff --cached --quiet; then
      echo "main is already at the released versions (a concurrent release landed it); nothing to push."
      exit 0
    fi
    git commit -m "ci: bump versions (${SUMMARY})"
  done
fi

# -------------------------------------------------------------------
# 4. Build release artifacts per target
# -------------------------------------------------------------------
# The server ships as a single static binary with zero system-library link deps
# (DESIGN Sec.1), so we build the musl targets — fully static, run anywhere,
# including a scratch container. amd64 and arm64 are both release targets
# (DESIGN Sec.12); arm64 (Raspberry Pi satellites) is first-class.
#
# The bundled static ffmpeg and the web UI travel in the CONTAINER IMAGES
# (built by server-build-and-deploy.yaml, multi-platform), which are the
# batteries-included artifact. These tarballs carry just the ichoi binary.
build_target_artifacts() {
  target=$1
  version=$2
  out=$3
  case "${target}" in
    server)
      # Real aarch64 musl cross toolchain (a glibc cross gcc mis-links musl's sqlite:
      # open64/stat64/mmap64 don't exist in musl). Native cross-compiler → ring's asm and
      # sqlite's C both build for aarch64-musl, no QEMU. Idempotent across target calls.
      if [ ! -x "${HOME}/aarch64-linux-musl-cross/bin/aarch64-linux-musl-gcc" ]; then
        wget -q https://musl.cc/aarch64-linux-musl-cross.tgz -O /tmp/aarch64-musl.tgz
        tar -xzf /tmp/aarch64-musl.tgz -C "${HOME}"
      fi
      export PATH="${HOME}/aarch64-linux-musl-cross/bin:$PATH"
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc
      export CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc
      for PAIR in "amd64:x86_64-unknown-linux-musl" "arm64:aarch64-unknown-linux-musl"; do
        ARCH="${PAIR%%:*}"
        TRIPLE="${PAIR#*:}"
        echo "=== Building ichoi ${version} for ${ARCH} (${TRIPLE}) ==="
        rustup target add "${TRIPLE}"
        # The server crate lives under server/ (its own Cargo workspace root).
        ( cd server && cargo build --release --target "${TRIPLE}" --bin ichoi ) 2>&1
        # Honor CARGO_TARGET_DIR (release.yaml points it at /tmp/ichoi-target); otherwise
        # cargo wrote to server/target.
        tar -czf "${out}/ichoi-${version}-linux-${ARCH}.tar.gz" \
          -C "${CARGO_TARGET_DIR:-server/target}/${TRIPLE}/release" ichoi
      done
      ;;
    *)
      echo "WARNING: no artifact builder for target '${target}'; releasing tag + notes only."
      ;;
  esac
}

# -------------------------------------------------------------------
# 5. Create a GitHub release per published target
# -------------------------------------------------------------------

# NOTE: do NOT guard on "tag already exists" here. `semver-tags run` (step 2) created and
# pushed each target's tag before we got here, so the tags ALWAYS exist at this point —
# guarding on them would skip release creation every time. The recovery pass above handles
# a tag left without a GitHub Release by an interrupted earlier attempt.
while read -r t v tag; do
  RELEASE_DIR="/tmp/release/${t}"
  mkdir -p "${RELEASE_DIR}"
  build_target_artifacts "${t}" "${v}" "${RELEASE_DIR}"

  if [ "${SKIP_GITHUB:-false}" = "true" ]; then
    echo "=== SKIP_GITHUB=true: skipping GitHub release for ${tag}; artifacts in ${RELEASE_DIR} ==="
    continue
  fi

  echo "=== Creating GitHub release ${tag} ==="
  if ls "${RELEASE_DIR}"/* >/dev/null 2>&1; then
    GH_TOKEN="${GITHUB_PAT}" gh release create "${tag}" \
      --repo "${REACTORCIDE_REPO}" \
      --title "${tag}" \
      --generate-notes \
      "${RELEASE_DIR}"/*
  else
    GH_TOKEN="${GITHUB_PAT}" gh release create "${tag}" \
      --repo "${REACTORCIDE_REPO}" \
      --title "${tag}" \
      --generate-notes
  fi
  echo "=== Released ${tag} ==="
done < /tmp/published-targets.txt
