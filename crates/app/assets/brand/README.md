# Dalan app artwork

The user supplied an Apple Icon Composer package, originally named `dalan-db.icon`. It is preserved unchanged as `Dalan.icon/`, including `icon.json` and its original PNG layer filename. This asset is product artwork, distinct from the Lucide utility/provider icons.

- `Dalan.icon/`: original editable Icon Composer source, including background and platform settings.
- `dalan.png`: 1024 × 1024 RGBA copy resized from the original PNG layer. Used by the README and as the portable artwork fallback; no background/gradient, new logo, or other design was invented.
- `Dalan.icns`: standard macOS multi-size icon generated from that PNG for compatibility.

Regenerate the PNG/icns using macOS's existing `sips` and `iconutil` tools:

```sh
./scripts/prepare-icons
```

Default bundling copies `Dalan.icns` and `dalan.png` to app Resources and sets `CFBundleIconFile`. No additional runtime icon assignment is required for normal bundle launches. The `.icon` package cannot be used just by copying it into Resources: it must be compiled by Xcode's `actool` to an `Assets.car` and paired with the returned icon-name metadata.

```sh
./scripts/macos --icon-composer --open
```

This opt-in path requires full Xcode 26+ supporting Icon Composer. It retains `.icns` for compatibility and adds `CFBundleIconName` only after compilation actually supplies `Assets.car` and valid metadata. Failure leaves the existing bundle unchanged. The minimum target passed to the icon compiler is 26.0; this is not a decision about Dalan's eventual minimum OS matrix.

The local development host has Command Line Tools but no `actool`, so PNG/icns generation and packaging are verified here; real Icon Composer output/rendering is not. Linux/Windows desktop packaging is not implemented; `dalan.png` is the reusable fallback when those targets are developed. The current fallback is the original PNG layer, not a reconstruction of Icon Composer's composited gradient/shadow effects.

The artwork is user-supplied. Its copyright/redistribution terms have not been specified; do not assume the project or third-party license grants apply to it. Do not distribute it without resolving those terms.
