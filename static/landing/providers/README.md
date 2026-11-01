# Provider wordmarks

Marks are provided by their respective owners. They appear when published terms
permit this placement or the project owner has explicitly chosen otherwise.
Artwork and exceptions are documented below. Other engines stay plain text.

Terms were measured on 2026-11-01 from the owner pages linked below, and the
official files were downloaded and hash-verified where available (see
Verification). Owner decisions on Cartesia, Microsoft AI, Gemini, Mistral,
OpenAI and the offline-row marks (KittenTTS, Qwen, Piper, Pocket TTS) are
recorded in their entries.

**Theming (2026-11-01): one SVG per provider, no color variants.** Each mark
ships as a single file; the landing page paints it through a CSS alpha mask
(`mask-image` + `background-color: currentColor`), so the same file serves the
light and dark themes and the files' internal fills never affect rendering.
The former `-black`/`-white` file pairs were deleted along with the
`provider-light`/`provider-dark` DOM plumbing.

**Optimization (2026-11-01): all shipped files are derived from the official
artwork, and none is a byte-exact copy anymore.** Every SVG went through svgo
4.1.0 (`--multipass`, default preset, `removeViewBox` disabled so mask sizing
keeps its viewBox, LF line endings). The official bytes each file derives from
stay pinned in Verification table A; each shipped file's post-transform hash
is in table B. Ink geometry was re-measured in a real browser after the pass
(`getBBox`): every mark's ink still matches its viewBox (see Ink verification).

## Marks shipped

- **OpenAI** (`openai.svg`; **one combined lockup file**, rendered at
  80.35×16 px). Source: the official kit
  `https://cdn.openai.com/brand/openai-logos.zip`, which the brand page's
  "Download logos" control serves after its "By using our logos, you agree to
  our Marks usage terms" checkbox; files `OAI_OpenAI-Blossom_Black.svg` and
  `OAI_OpenAI_Wordmark_Black.svg`. **Composition (2026-11-01, owner
  direction): the Blossom and the wordmark are merged into a single SVG** —
  the earlier side-by-side `<img>` pair (32×32 Blossom box + 12 px gap + 85×38
  wordmark box) read as two separated marks. The merge scales the wordmark's
  ink ×1.5573 so its ink height equals the Blossom's ink height (354.674
  viewBox units, previously 15.85 vs 15.97 px — unequal), places it after a
  106.4-unit ink-to-ink gap (0.30 × ink height), and crops the viewBox to the
  combined ink (1781.13×354.67): the mark fills its box, so no ink-center
  nudge is needed. The owner-baked clear space inside the old wordmark file
  was removed by the crop; the strip's own layout clear space exceeds it
  (32 px gaps, 16 px vertical padding vs ~12.8 px horizontal / ~11 px
  vertical equivalents at render scale). Ink measured 847.66×227.75 (wordmark,
  1212×542 viewBox) and 354.67×354.67 (Blossom, 716×716 viewBox, centered) in
  a browser (`getBBox`, 2026-11-01). The kit ships no combined lockup file,
  and OpenAI's own site header currently uses the wordmark alone (header ink
  ratio 287.5/77.35 = 3.717 matches the kit wordmark's 847.66/227.75 =
  3.722, and its six ink segments are the six letters), so the lockup —
  symbol, scale, gap and crop — is **ours: a judgment call, flagged for
  owner review** (an earlier decision the same day had picked wordmark-only,
  superseding a Blossom-only decision before that; the final merge decision
  supersedes both). Terms ([brand page](https://openai.com/brand/), measured
  2026-11-01): use the logo "only when it directly relates to OpenAI
  services", "exactly as provided", never more prominently than your own
  marks, never implying endorsement; OpenAI may review and require changes
  and may terminate permission at any time. The merged file departs from
  "exactly as provided" by construction; it stays because the provider row
  directly references the supported OpenAI engine — it is not a partnership
  lockup (co-branded lockups do require bilateral approval, and none is
  used). svgo then baked the transform into the path data (single path,
  render-equivalent; geometry re-verified, see Ink verification). Permission
  route: partnercomms@openai.com; legal: legal@openai.com.
- **ElevenLabs** (`elevenlabs.svg`; 694×90 viewBox, 16 px render height).
  Source: [brand page](https://elevenlabs.io/brand), official file
  `elevenlabs-logo-black.svg`. The page permits the official files and
  requires them unaltered, with clearance equal to the logo height. The
  shipped file is the official black download, renamed (the `-black`/`-white`
  pair collapsed to one mask-themed file) and svgo-optimized — a departure
  from "unaltered" flagged for owner review. The row provides one
  logo-height of clear space on every side (16 px padding + 32 px gaps).
- **Fish Audio** (`fish.svg`; 1285×161 viewBox, cropped to the mark, 16 px
  render height). Source: official `wordmark-clearspace.svg` from the
  [brand guidelines](https://docs.fish.audio/developer-guide/resources/brand)
  (Google Drive assets, agreed to the usage guidelines), re-verified
  2026-11-01 against the live official copies the docs page serves from its
  own CDN (`https://mintcdn.com/hanabiaiinc/.../branding/light/wordmark-clearspace.svg`):
  every shipped path existed verbatim in the official file, and the removals
  were exactly the official file's extra overlay paths. The guidelines state:
  "You may freely use and redistribute our brand assets when referencing Fish
  Audio," and ask for a link to fish.audio, which the landing mark provides.
  The clearance-diagram overlays (`Clearance Marker`, `Border`) were removed;
  a search of the shipped file finds neither. The mark paths, colors and
  proportions are otherwise unchanged. Clear space: the guidelines set the
  wordmark minimum at the height of the letter "F"; the letter F is shorter
  than the 16 px mark (not measured), and the strip's layout gaps exceed
  that. Now additionally svgo-optimized.
- **Cartesia** (`cartesia.svg`; 1102×156 viewBox, 16 px render height).
  Source: [brand page](https://www.cartesia.ai/brand), official file
  `/brand/cartesia-logo-black.svg` (the `-black`/`-white` pair collapsed to
  one mask-themed file). The page requires approved forms only (dark on
  light, light on dark) with no alteration, and says its guidelines are
  written for partners, press and developers. It states no clear-space figure
  and grants no explicit license for third-party provider lists. **Owner
  decision (2026-11-01): the mark stays.** The shipped file is now the
  official black download renamed + svgo-optimized — a departure from "no
  alteration" flagged for owner review alongside the placement itself.
  Permission route: the contact page on the brand site.
- **SpaceXAI (xAI)** (`xai.svg`; 834×318 viewBox, tight ink, 16 px render
  height). Source: the official kit
  `https://data.x.ai/logos/SpaceXAI_Grok_Assets.zip`, file
  `spacexai - symbol - black - transparent.svg` (the `-black`/`-white` pair
  collapsed to one mask-themed file). Terms
  ([brand guidelines](https://x.ai/legal/brand-guidelines), measured live
  2026-11-01): marks may be used "only to accurately refer to us or our
  services"; logos only "exactly as provided at the download link below,
  without any alteration or adjustment"; the owner may require changes after
  review and may terminate permission at any time. The shipped file is now
  the official black download renamed + svgo-optimized — a departure from
  "exactly as provided" flagged for owner review alongside the placement.
  The row is an accurate reference to the supported engine, so the use falls
  under the published terms. Naming note: the guidelines prose and site
  footer say "SpaceXSI" ("SpaceX Super Intelligence LLC"; page dated
  February 14, 2025, © 2026), the kit zip and its files say "SpaceXAI", and
  the engine/API stay branded xAI (api.x.ai). The row shows the kit symbol
  with alt "SpaceXAI" and links to https://x.ai. Legal contact: legal@x.ai.
- **Microsoft AI, "MAI" mark** (`mai.svg`; 56×26 viewBox with ~54.5×24.2 of
  ink, rendered at 37.1×17.2 px so the ink sits at the band's ~16 px height —
  a 16 px box rendered it at 14.9 px ink, off the band standard; row label
  "Microsoft AI", links to https://microsoft.ai). Source: the inline SVG logo
  in the [microsoft.ai](https://microsoft.ai/) homepage markup, retrieved
  2026-11-01. The three path elements are copied unchanged; only
  `currentColor` was replaced with black. **Owner-terms conflict, placed on
  the project owner's decision:** Microsoft's
  [trademark guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks)
  say "Many uses, including our logos ... will require a license first," and
  bar using Microsoft logos "in any manner" without a written license. No
  license was obtained. **Owner decision (2026-11-01): the row and mark
  stay.** Permission route: trademarks@microsoft.com. The row previously
  named the Azure AI Speech engine; the MAI mark is the Microsoft AI
  division's mark, so the row now reads "Microsoft AI".
- **Google Gemini** (`gemini.svg`; 288×65 viewBox, rendered at 71×16 px).
  Source: inline header SVG from the official
  [Gemini site](https://gemini.google/us/about/), retrieved 2026-11-01.
  Extracted artwork, not byte-exact downloads. **Monochrome flattening
  (2026-11-01, owner direction: all marks black/white):** the six wordmark
  letter paths and the sparkle geometry are unchanged from the source
  serialization, but the site's gradient rendering was flattened — the
  sparkle's gradient fill and its white/black underlay became a single solid
  fill, and the masked blurred colour-blob layer behind the sparkle (10
  ellipses, 11 Gaussian-blur filters, one alpha mask, one linear gradient)
  was removed; no chromatic fill or filter remains. Source serialization
  SHA-256:
  `a854d4b5afab1139e913786b64c64382d9b68fb8f566f53b2a9f10fdddcd1f61`.
  Now additionally svgo-optimized. Google's
  [non-partner guidance](https://about.google/brand-resource-center/guidance/partnerships/)
  says "We don't allow our logo to be used by individuals or companies who do
  business with Google on a limited basis but are not formal partners."
  **Owner decision (2026-11-01): use the Gemini logo despite this
  restriction**; the owner approved extraction when a standalone official
  file could not be found. No Gemini-specific clear-space figure was found;
  the list provides 16 px vertical padding and 32 px gaps between items.
- **Mistral** (`mistral.svg`; 98×22 viewBox, ink measured 91.06×16.05 in a
  browser (`getBBox`, 2026-11-01), rendered at 98×22 so the ink sits at
  ~16 px height). Source: the inline header logo lockup in the
  [mistral.ai](https://mistral.ai/) homepage markup, retrieved 2026-11-01 —
  the pixel-M lockup (10 rects + 8 wordmark paths, viewBox 98×22). Extracted
  artwork, not byte-exact downloads: the site's version carries the official
  rainbow gradient (5 chromatic rect fills, `#FFAF01` … `#C4001D`) and
  `currentColor` wordmark text; every rect and text fill was replaced with a
  single solid fill and nothing else changed. Source serialization SHA-256:
  `4af90f5941c959b554b4f3eb82c5633044ad06fdf2d8daa837eea0357fbd7e22`. Now
  additionally svgo-optimized. The brand-page kit
  (`https://cms.globalaegis.net/api/documents/file/Mistral_Logos_2026.zip`,
  and the direct PNG files on the same host) stayed Cloudflare-blocked on
  2026-11-01 even to a real Chrome session, so the homepage lockup is the
  official source in use; the homepage itself was reachable. Terms
  ([brand page](https://mistral.ai/brand/), measured 2026-11-01): the page
  welcomes "partners and collaborators" and says nothing about product or
  provider lists; the commercial terms (prior session lead, not re-checked)
  require written approval for publicity. **Owner-terms caveat, placed on the
  project owner's decision:** no written approval was obtained; the row is an
  accurate reference to the supported Mistral engine. Permission route: the
  contact page on the brand site.

The four entries below belong to the offline (local engines) group of the
provider carousel. Placement decisions follow the same
pattern as the cloud entries above: where no published terms permit the
placement, the project owner decided to proceed, and that decision is
recorded in the entry.

- **KittenTTS** (`kitten.svg`; composed lockup 5583.5×1000 viewBox, tight
  ink, 100.5×18 px render). **This is a composed mark, flagged for owner
  review** — the owner publishes no lockup or wordmark asset (checked
  2026-11-01: kittenml.com and platform.kittenml.com serve the symbol only,
  the repo README carries only a marketing banner, HF and GitHub carry no
  text lockup; a 188×58 owner-side lockup image supplied by the project
  owner is too low-res to ship from). The composition follows the owner's
  reference image and the same recipe as the OpenAI lockup above:
  - **Symbol**: the official site asset
    [kittenml-mark.svg](https://kittenml.com/assets/images/kittenml-mark.svg)
    (2048×2048). The file is layered two-variant art (black line art over a
    white face underlay with cream/white accents); **monochrome conversion:
    every path is repainted in document order into an SVG `<mask>`** — dark
    fills become ink, light fills carve — reproducing the printed
    (light-theme) form; heart-eye glints and cream accents become transparent
    holes; the `fill="none"` background rect is dropped.
  - **Text**: "Kitten TTS" set in **Geist SemiBold** — the owner's own hosted
    brand sans (`kittenml.com/assets/fonts/geist-600.woff2`, the face behind
    their `--sans` stack; the site's logo block itself pairs this mark with
    live text). Glyph outlines are extracted from the font file into path
    data (harfbuzz-shaped, kerned; the file ships no `<text>` elements).
  - **Layout**: the cat dominates — text ink height = 0.65× cat ink height,
    vertically centered in the cat's ink band; gap 0.35× cat ink height
    (owner header ratio 10/27 ≈ 0.37, reference image ≈ 0.33); render box
    100.5×18 px — the whole lockup scaled up from the band's 16 px so the
    smaller text stays readable. History, same day: the first pass used equal
    ink heights (cat = text cap height, the OpenAI recipe); Ilya's review
    directed the head-dominant ratio and the 18 px render. The deviation from
    the owner reference's ~⅓ text ratio is smaller now but still ours (⅓
    would be unreadable at band size). The cat's source ink-box origin is
    translated to the lockup origin before scaling; without that translation
    the cat shifts down and right and is clipped. svgo-optimized. Terms ([platform terms](https://kittenml.com/terms/)
    §6.02, measured 2026-11-01) require prior written permission for
    associated logos; none was obtained — **owner decision (2026-11-01): the
    composed mark ships**, as an accurate reference to the supported engine,
    not an endorsement. (History, same day: a symbol-only file shipped first
    and was removed after owner visual review, then restored as this lockup.)
- **Qwen** (`qwen.svg`; 853×139 viewBox wrapping an embedded raster, 98.2×16 px
  render). Source: the official
  [Qwen3-TTS repo logo](https://qianwen-res.oss-cn-beijing.aliyuncs.com/Qwen3-TTS-Repo/qwen3_tts_logo.png)
  (1251×313 transparent PNG, retrieved 2026-11-01) — the lockup of the exact
  engine the row links (hexagon symbol + "Qwen3-TTS" wordmark). Earlier the
  same day the row shipped the vector three-stroke mark from
  [QwenLM/qwen-code PR #14052](https://github.com/QwenLM/qwen-code/pull/14052)
  (`packages/desktop/bootstrap/qwen-code-logo.svg`, flat blue `#082DFF`,
  square viewBox kept); **owner visual review (2026-11-01) rejected the
  symbol-only form** — it reads as an abstract cluster at band size — so the
  row now uses the wordmark line of the official engine logo instead.
  **Crop and raster embed (2026-11-01): the shipped file contains only the
  "Qwen3-TTS" wordmark line** — the hexagon symbol's white overlays would blob
  under the alpha mask (the same reason Pocket's mascot is excluded). The
  crop is downscaled to 64 px height (4× the band height, Lanczos), quantized
  to a 32-color RGBA palette (4.3 KB) and embedded as a base64 data URI inside
  an SVG wrapper; svgo passes it through unchanged. No terms permitting this
  placement were found; the Qwen Chat
  [service agreement](https://chat.qwen.ai/legal-agreement/terms-of-service)
  requires prior written consent for its marks but is service-specific, and
  Apache-2.0 code licensing does not establish logo permission. **Owner
  decision (2026-11-01): the mark stays.**
- **Piper** (`piper.svg`; viewBox cropped to the measured ink 114.36×35.69,
  51.27×16 px render). Source: the owner-published
  [logo.svg](https://github.com/OHF-Voice/piper1-gpl/blob/057093803be0aa53153abc9d5be9ea4ddec378f8/etc/logo.svg)
  in OHF-Voice/piper1-gpl (at commit `0570938`, retrieved 2026-11-01) — an
  Inkscape file whose 14 paths are all solid `#000000`, so the artwork masks
  cleanly with no internal color to flatten. Transformations: viewBox cropped
  to ink (the file bakes a 2-unit Inkscape margin) and svgo (strips the
  Inkscape metadata, −64.5%). No Piper-specific brand terms were found; the
  Open Home Foundation's
  [brand-assets notice](https://github.com/OpenHomeFoundation/brand-assets)
  does not list Piper, and GPL-3.0 code licensing does not establish logo
  permission. **Owner decision (2026-11-01): the mark stays.**
- **Pocket TTS** (`pocket.svg`; 643×100 viewBox wrapping an embedded raster,
  102.9×16 px render). Source: the official repo artwork — the
  "pocket-tts-logo-v2-transparent" PNG (1446×622) at the top of the
  [pocket-tts README](https://github.com/kyutai-labs/pocket-tts/blob/91a9e96433bfe23375eea719b03f0081466f8e20/README.md)
  (GitHub user-attachments, retrieved 2026-11-01). The lockup is mascot + a
  small `/kyutai` label + the "Pocket TTS" wordmark line. **Crop and raster
  embed (2026-11-01): the shipped file contains only the wordmark line** — at
  band height the mascot renders as a shapeless blob and the `/kyutai` label
  as ~2 px of noise. The crop is downscaled to 64 px height (4× the band
  height, Lanczos), quantized to a 32-color RGBA palette (3.3 KB) and embedded
  as a base64 data URI inside an SVG wrapper (one file per owner; the CSS
  alpha mask reads the raster's alpha). svgo passes it through unchanged. The
  Kyutai corporate wordmark
  ([LOGO_CLEAN](https://kyutai.org/_next/static/media/LOGO_CLEAN.5855f56f.svg),
  hash pinned in table A) was evaluated and rejected: it identifies Kyutai
  rather than Pocket TTS, and its thin stroke is unusable at 16 px. No
  applicable brand-use terms for this placement were found; the MIT code
  license is not a mark license. **Owner decision (2026-11-01): the mark
  stays.**

The marks identify supported integrations; they do not imply endorsement or
partnership. The providers strip is ONE continuous carousel line: the Cloud
and Local group labels are inline separators inside it (`providers.svelte`).
List padding and gaps reserve at least 16 px clear space around the marks
(32 px gaps, 16 px vertical padding). The marks
render through a CSS alpha mask painted with the text color
(`mask-image` + `currentColor` in `landing.css`), so the files' internal
fills are presentation-irrelevant. The line is a marquee carousel: both
edges fade out behind a static CSS mask, the loop is a duplicate of the same
line, and `prefers-reduced-motion` stops the animation (the line becomes
hand-scrollable instead).

## Plain text, with the reason

- **Kokoro** ([repo](https://github.com/thewh1teagle/kokoro-onnx)):
  kokoro-onnx is MIT and the kokoro model is Apache-2.0. No official mark or
  applicable brand-use terms were found in the wrapper or original model
  sources. Plain text.

Edge remains excluded because it is retired from the UI; uv is a
prerequisite, not a speech engine. The lists track
`src/lib/components/engine/engine-meta.ts` otherwise.

## Verification (2026-11-01)

**Table A — official source bytes (provenance).** These are the sha256
values of the owner-side files each shipped mark derives from. No shipped
file matches them anymore (everything is renamed, merged, flattened or
svgo-optimized); they pin the official bytes so any future re-derivation can
be checked against the source.

```
elevenlabs-logo-black.svg        7a41b7f26b6286f607d34c80a658ef06e5cb6c094a1b087b563c8508e15f2178
cartesia-logo-black.svg          d3205239daccd54b6705f331d72bd14fb2c5b378d3201d838c11237cbcf8b84b
spacexai symbol black            435f412250a243399c6af46c83dbbc720d55e29dc4570fc046d9bec254d998a2
OAI_OpenAI_Wordmark_Black.svg    6b5116a32769087308e1acee6f5f593e8989addc760f69dd37f59e54a3926d2d
OAI_OpenAI-Blossom_Black.svg     75c1e9fffa5e8c437bec1d67197a73992bca45d166c6ff23215185dea8fae92a
gemini header svg (extract)      a854d4b5afab1139e913786b64c64382d9b68fb8f566f53b2a9f10fdddcd1f61
mistral header svg (extract)     4af90f5941c959b554b4f3eb82c5633044ad06fdf2d8daa837eea0357fbd7e22
kittenml-mark.svg                2af991d265e01d383d97e0eb7cb6731f6bca866e5e2b298109ecc7abab3b5a76
geist-600.woff2 (typeset source) ece6e8814750aba251b0670294a450b810a191c1dfe148df8c671916bee0de8c
piper1-gpl logo.svg              430a1ebf38b41479bf4052d6022129fa6c2a1c7f7c7cfe81ceef0e5d4dcda15b
qwen3_tts_logo.png               ca98fed49d16e7d8c8736ee83a18a24183bb3246717d3e06943862b25a2852d0
pocket-tts-logo.png              15ff399a51fb2a9ab06ff9ddb6e367fa2aea6f911ebad86a685e7117c65b5646
qwen-code-logo.svg (superseded)  24156c849c9124505e13b38c6583a9d5d85a1f57d996c324d967f480a4d88696
kittenml_logo.svg (same mark)    a48af0c5695e1aa25e04273d9e300e79722132090adc34b0a339e5d81438c7b8
kyutai LOGO_CLEAN.svg (rejected) 92efe833abac535fb87bc1ef65a8d5fc8c2b10e627ef180f0de85df68e118de3
```

(Formerly byte-exact, now deleted with the variant pairs:
`elevenlabs-white` e9643312…, `cartesia-white` 299c91dd…, `xai-white`
ce4ba444…, `OAI_OpenAI_Wordmark_White` 5dcfbca5…, `OAI_OpenAI-Blossom_White`
01d15876….)

Kit archive: `SpaceXAI_Grok_Assets.zip`
db9129acd4efc4c2202d25afe31b70281a79f8507f75520ab5e6b3356895a7e9.

**Table B — shipped files (post-transform).** Each file's sha256 as it
exists on disk after the transformations described in its entry above
(rename / merge / flatten / svgo 4.1.0):

```
cartesia.svg    3022 bytes  7df62c6a2972b0fe30bc8c0aa1732b925b975dae6c92433d2ed0d316d8e476fc
elevenlabs.svg  2419 bytes  628edb0c9cc1ad18e8accbe618665df75de02ba4cbc1d01db587667776abfb14
fish.svg        4468 bytes  f8a273e0e4b3804acabd7f496bc44ebba5d6f2fd7feb44b7e3e47828728e02ac
gemini.svg      3040 bytes  bf43ee08a70bc21ea65ae05fd01b2109da4fb3c8f325382ad72fe47347ab425a
kitten.svg     11088 bytes  6939b1db05661e5206fe821e7b4e3657e24dc64579ab44bf25c391125489530f
mai.svg         1158 bytes  cfa44c6ea30cad5db4d86955e6ac9e0e0e18a9ae08985ccce9ba0549226abee2
mistral.svg     2629 bytes  43cbcfa800aaf922ade071e1158a39dc002975652193efe7e4e97b8cabbb6657
openai.svg      3324 bytes  2fc6085ca46be6f548e34bb4bba2dc0185b88b2e8c0f1d3d41f8a974416141b9
piper.svg       3003 bytes  2b90b74d95d3740bb8d129613d9dcf1e37420dbcf966eee9b73e2366712f8645
pocket.svg      4587 bytes  fe03fa84b2495cfcab3b26821b2394216a4811c16f8b9acd5dad4882f9afd5ee
qwen.svg        5815 bytes  05b4ae2950890b5467bbcc6ebb1c013e8cb662de781124d4ef826f3c38e7f495
xai.svg          535 bytes  1cd5c0c46378486fa0019d7d2f28736070cf638c128944e727b4da257c0e0881
```

**Ink verification (`getBBox`, real browser, 2026-11-01, post-svgo).** Each
mark's ink matches its viewBox within ≤0.01 units, so the render boxes in
`providers.svelte` deliver ~16 px ink: cartesia 1102×156 tight; elevenlabs
694×90 tight; fish 1284.6×160.9 in [118,119,1285,161]; gemini 288×64.9
tight; mai 54.46×24.16 in 56×26 (hence the 37.1×17.2 box); mistral
91.06×16.05 in 98×22; openai 1781.13×354.68 tight (= viewBox; composed
width = Blossom ink + 106.4 gap + scaled wordmark ink, no overlap); xai
831.48×317.7 in 834×318. Offline row: kitten has a 5583.5×1000 viewBox,
its cat translated from the source ink-box origin before scaling. A browser
raster probe (1565×280) shows the cat ink spanning the full height and the
0.65×-cat-height text ink vertically centered (rows 49–230 vs expected
49–231); `getBBox` alone cannot account for mask cutouts. The shaped text
has a 0.35× cat-ink-height gap from the cat's ink box; piper
114.36×35.69 tight (= viewBox); qwen 853×139 tight (= viewBox, raster
wrapper, wordmark line of the official engine logo); pocket 643×100 tight
(= viewBox, raster wrapper). The revised kitten and the merged single-line
carousel await owner review (kitten at 18 px) in both themes. The pre-svgo combined
OpenAI build is regenerated
deterministically from the kit files by
`%LOCALAPPDATA%/hermes/cache/scratch/logo-downloads/build-openai.py` (24 h
pruned; re-create from the recipe in that script's header). The offline-row
builds are regenerated the same way by
`…/logo-downloads/offline/build-offline.py` and
`…/logo-downloads/offline/build-kitten-lockup.py` (sources re-downloadable
from the URLs in the entries; the lockup script additionally needs
fonttools + brotli + uharfbuzz under `uv run`; svgo via
`…/logo-downloads/svgo.config.cjs`).

## Verification gaps

- Owner visual review is pending for: the mask+currentColor theming in both
  themes, the merged OpenAI lockup (single combined SVG, 0.30×-ink-height
  gap — the gap value is a tunable constant in the build script), the MAI
  resize to ~16 px ink, carousel speed and edge fades (now one merged
  Cloud/Local line at 90 s), the SpaceXAI
  symbol against "just the last xai letter logomark" (the symbol is
  thin-stroked and reads faint at 16 px — observed, not yet owner-reviewed),
  and the offline-row raster embeds (qwen, pocket): alpha edges quantized to
  a 32-color palette, slightly softer than the vector marks when zoomed
  (readable at band size in both themes in the browser check). The
  KittenTTS composed lockup is owner-review-pending by construction (no
  official wordmark exists — see its entry); at band size the cat reduces
  to a badge silhouette while the typeset text carries recognition, and the
  0.65× text ratio (Ilya-directed head-dominant revision) still departs
  from the owner reference's ~⅓ text ratio.
- Fish's clearance minimum ("height of the letter F") is reasoned, not
  measured.
- The Mistral commercial terms (written-approval requirement) come from a
  prior session's lead and were not re-checked on 2026-11-01.
