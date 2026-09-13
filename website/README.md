# Ichoi project website

This directory contains the Ichoi static website. Edit files in `site-src/`.
Pysocha writes the deployable output to `site/`. Do not edit `site/` by hand.

## Build

The build uses Pysocha at commit `18b2d6e704ba63e80f82d287b5831c1151c388a4`:

```sh
./tools.sh build
```

Run the command twice. The second run must make no changes. Preview the result
with `python3 -m http.server 4173 --directory site`.

## Deployment

Run `./tools.sh check-deploy` before deployment. It fails while the reserved
`.invalid` domain or contact placeholders exist. A person must approve the
privacy and support contacts before these values change.
