# Ichoi project website

This directory contains the Ichoi static website. Edit files in `site-src/`.
Pysocha writes the deployable output to `site/`. Do not edit `site/` by hand.

In a repository checkout, the build copies approved logos, favicons, the touch icon, and the social preview
from `../assets/brand/` into `site-src/content/extra_files/assets/`. Do not edit
those copies. To copy the assets without a build, run
`python3 ../scripts/install-brand-assets.py --target website` from this directory.
The header selects the approved light or dark mark for the system appearance.
The Docker build uses `website/` as its context and uses the checked-in copies.

## Build

The build uses Pysocha at commit `18b2d6e704ba63e80f82d287b5831c1151c388a4`:

```sh
./tools.sh build
```

Run the command twice. The second run must make no changes. Preview the result
with `python3 -m http.server 4173 --directory site`.

## Deployment

The site is served at `https://catalystichoi.com`. The version comes from the
`website/vX.Y.Z` Git tag. Do not edit `version/VERSION.txt` by hand.

1. A pull request merges to `main`.
2. semver-tags reads the conventional commits that changed `website/`. A `fix:`
   commit makes a patch release. A `feat:` commit makes a minor release. Other
   types, such as `docs:` or `ci:`, make no release.
3. For a release, CI pushes the `website/vX.Y.Z` tag. Then it writes the version
   to `version/VERSION.txt` in a separate commit.
4. That commit starts the deployment.

A merge with no `fix:` or `feat:` commit in `website/` does not deploy.

`./tools.sh check-deploy` fails while a reserved `.invalid` domain or a
`REPLACE-ME` value exists. The image build runs the same check.

The pysocha-site chart makes an Ingress. The deploy job turns it off and
applies `httproute.yaml`, because the Ingress conflicts with the cluster
Gateway listeners. See `.reactorcide/BOOTSTRAP.md`.
