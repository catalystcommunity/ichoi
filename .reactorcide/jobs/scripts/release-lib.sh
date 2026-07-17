#!/bin/sh

# Add the newest reachable tag for each target when semver-tags already pushed
# it, but the corresponding GitHub Release was never created. This is the state
# left behind when a release job fails after tagging (for example, while
# stamping VERSION.txt). Keeping recovery separate makes the state transition
# unit-testable without a network or a real GitHub repository.
recover_unreleased_targets() {
  targets=$1
  published_file=$2
  repository=$3

  for target in ${targets}; do
    if grep -q "^${target} " "${published_file}"; then
      continue
    fi

    latest_tag=$(git tag --merged HEAD --list "${target}/v*" --sort=-v:refname | head -1)
    if [ -z "${latest_tag}" ]; then
      continue
    fi

    if GH_TOKEN="${GITHUB_PAT}" gh release view "${latest_tag}" --repo "${repository}" >/dev/null 2>&1; then
      continue
    fi

    version=${latest_tag#"${target}/v"}
    case "${version}" in
      ''|*[!0-9.]*|.*|*.)
        echo "ERROR: refusing to recover malformed release tag '${latest_tag}'." >&2
        return 1
        ;;
    esac

    echo "${target} ${version} ${latest_tag}" >> "${published_file}"
    echo "=== Recovering incomplete release: ${target} -> ${latest_tag} (${version}) ==="
  done
}

