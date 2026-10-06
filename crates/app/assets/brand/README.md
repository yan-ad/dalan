# Dalan app artwork

The user supplied the updated `dalan-db.icon` Apple Icon Composer package. Dalan uses its **Dalan New Transparent.png** layer: the winding-path database mark in **sRGB `#FF6AEB`** on a **solid black `#000000` background**. This is product artwork, distinct from Lucide utility/provider icons.

- `Dalan.icon/Assets/Dalan New Transparent.png`: unchanged supplied pink alpha layer. The unused white duplicate is not included.
- `Dalan.icon/icon.json`: adapted package settings use only the pink layer, solid black fill, no shadow and no translucency, preserving its platform settings. The user's original Downloads package is not modified.
- `dalan.png`: reproducible 1024 × 1024 **opaque** sRGB fallback. `scripts/compose_icon.py` uses the supplied alpha shape, exact `#FF6AEB` foreground and black background; antialiased edge pixels blend those colors. No silhouette is redrawn.
- `Dalan.icns`: standard macOS multi-size icon generated from the composed PNG.

Regenerate the PNG/icns using macOS's existing `sips` and `iconutil` tools:

```sh
./scripts/prepare-icons
```

Default bundling copies `Dalan.icns` and `dalan.png` to app Resources and sets `CFBundleIconFile`. No additional runtime icon assignment is required for normal bundle launches. The `.icon` package cannot be used just by copying it into Resources: it must be compiled by Xcode's `actool` to an `Assets.car` and paired with the returned icon-name metadata.

```sh
./scripts/macos --icon-composer --open
```

This opt-in path requires full Xcode 26+ supporting Icon Composer. It retains `.icns` for compatibility and adds `CFBundleIconName` only after compilation actually supplies `Assets.car` and valid metadata. Failure leaves the existing bundle unchanged. The minimum target passed to the icon compiler is 26.0; this is not a decision about Dalan's eventual minimum OS matrix.

The local development host has Command Line Tools but no `actool`, so PNG/icns generation and packaging are verified here; real Icon Composer output/rendering is not. Linux/Windows desktop packaging is not implemented; `dalan.png` is the reusable fallback when those targets are developed. The fallback is the flat black/pink composite; Apple may apply platform icon masking when rendering the compiled source. Real Icon Composer output remains unverified on this host.

The artwork is user-supplied. Its copyright/redistribution terms have not been specified; do not assume the project or third-party license grants apply to it. Do not distribute it without resolving those terms.
