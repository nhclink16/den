# Bundled iOS resources

## Themes

`themes.json` is a byte-for-byte build resource copied from `crates/den-core/src/themes.json`. The app decodes it into the generated `DenAPI.Components.Schemas.Theme`, not a separate palette model. Run `python3 apps/ios/scripts/sync-resources.py` after changing the shared JSON.

## Fonts

All 15 font families used by the eight built-in themes are bundled as unmodified TTF files. Static families include regular, semibold and bold with matching italics where upstream provides them; variable families retain their upstream axes and italic files. The total is 35 TTF files, about 10 MiB.

The official Google Fonts binary distribution is pinned to [`809e4d8b8d7e9364a914909bb777679606c178b8`](https://github.com/google/fonts/tree/809e4d8b8d7e9364a914909bb777679606c178b8). Each family preserves its upstream `METADATA.pb` and `OFL.txt`. `Fonts/manifest.json` records the exact binary versions and SHA-256 hashes plus upstream source commits. Google Fonts may build distribution binaries from those upstream sources, so the distribution commit and binary hash identify the shipped bytes.

`font-families.json` maps theme family names to the binary default PostScript name. The default name is not necessarily regular weight. `DenFonts` registers the files process-locally and resolves a regular-weight descriptor before creating a Dynamic Type-aware font. This matters for Manrope, Fira Code, Source Code Pro and Bricolage Grotesque. Unknown custom-theme font families fall back to the system sans or mono font.

| Family | Binary version | Upstream source commit |
|---|---|---|
| Zilla Slab | Version 1.1 | [6dcec520f9b2](https://github.com/mozilla/zilla-slab/tree/6dcec520f9b23ad7f380b7ce3e8072be95fe0004) |
| Atkinson Hyperlegible | Version 1.006 | [1cb311624b2d](https://github.com/googlefonts/atkinson-hyperlegible/tree/1cb311624b2ddf88e9e37873999d165a8cd28b46) |
| IBM Plex Mono | Version 2.3 | [9ab3b5b3b963](https://github.com/googlefonts/plex/tree/9ab3b5b3b96325fb20f365ee0804adca92024cbf) |
| Gabarito | Version 1.000 | [1f3fb39d6449](https://github.com/naipefoundry/gabarito/tree/1f3fb39d6449eefa880543f109f33ede0cd4064f) |
| JetBrains Mono | Version 2.211 | [19371302b95d](https://github.com/JetBrains/JetBrainsMono/tree/19371302b95d218af43299bce79ddbddd0bc364d) |
| Sora | Version 2.000 | [7f9a9c5d0ccd](https://github.com/sora-xor/sora-font/tree/7f9a9c5d0ccd1c099cfac420aa27133df1c5fdc4) |
| Inter | Version 4.001 | [66647c0bbbe4](https://www.github.com/rsms/inter/tree/66647c0bbbe41a850d79d9c76fb13add3378940f) |
| Bricolage Grotesque | Version 1.001 | [84745e5b9626](https://github.com/ateliertriay/bricolage/tree/84745e5b96261ae5f8c6c856e262fe78d1d6efdd) |
| Fira Code | Version 5.002 | [8da49d55f8b5](https://github.com/tonsky/FiraCode/tree/8da49d55f8b5978c5f888dd85452b79aad16cca2) |
| Manrope | Version 4.504 | [6f81ebecdf65](https://github.com/aaronbell/manrope/tree/6f81ebecdf65e4463b798cc07b16a4f8d5216917) |
| Source Serif 4 | Version 4.004 | [b3980ade53bb](https://github.com/adobe-fonts/source-serif/tree/b3980ade53bb3d023a0006076d05349236b309b1) |
| Source Sans 3 | Version 3.052 | [272b22b02e09](https://github.com/adobe-fonts/source-sans/tree/272b22b02e097e8eff1372111f88b5ab6063499f) |
| Source Code Pro | Version 1.026 | [803b7e23ec97](https://github.com/adobe-fonts/source-code-pro/tree/803b7e23ec97ae58b6232ea76519a76d428ba268) |
| Geist | Version 1.800 | [a6d260e6cbc0](https://github.com/vercel/geist-font/tree/a6d260e6cbc07eafdfad438f33601fe3c38b1e6f) |
| Geist Mono | Version 1.701 | [77f0563c0300](https://github.com/vercel/geist-font/tree/77f0563c03009d6c15c6342183fa53b352255b22) |

## Doorway assets

The app icon is rendered from `apps/desktop/icon.svg`, with no redraw or generated artwork. Its dark background is flattened to opaque RGB for the iOS asset catalog; iOS supplies the outer app-icon mask. The launch image uses the same mark paths. `LaunchBackground` and `LaunchMark` have light and dark variants using Den's shared `bg`, `ink` and `accent` roles. The operating system chooses launch-screen appearance before account preferences load.

Set the launch screen color name to `LaunchBackground` and image name to `LaunchMark` in the app target. Include `Fonts` as a folder resource to retain per-family licenses and avoid duplicate `OFL.txt` copy destinations. Include `themes.json` and `font-families.json` at the bundle root.

## Reproduction

- `python3 apps/ios/scripts/sync-resources.py --check` verifies resources offline without modifying files. This needs Python 3 only.
- `python3 apps/ios/scripts/sync-resources.py` refreshes shared JSON and renders SVG assets using `rsvg-convert` from librsvg.
- `python3 apps/ios/scripts/sync-resources.py --fetch-fonts` restores missing or changed font binaries, licenses and metadata from their pinned HTTPS URLs, checks hashes, then refreshes generated resources.

The app has no network font loader and no rendering-tool dependency at runtime.
