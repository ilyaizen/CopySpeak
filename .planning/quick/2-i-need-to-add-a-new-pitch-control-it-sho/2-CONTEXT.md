# Quick Task 2: Pitch Control + Quick Settings - Context

**Gathered:** 2026-03-06
**Status:** Ready for planning

<domain>
## Task Boundary

Add a client-side Pitch control (NOT engine-based) to Playback Settings (below Speed), and add both Speed and Pitch as Quick Settings on the Play/front page as compact sliders.

</domain>

<decisions>
## Implementation Decisions

### Pitch Range & Display
- Range: 0.5× to 2.0× ratio (displayed as multiplier, e.g. "0.75×", "1.0×", "1.50×")
- Default: 1.0× (no pitch shift)
- Internal unit: cents for Web Audio API (convert: cents = 1200 * log2(ratio))

### Client-Side Pitch Method
- Use Web Audio API `AudioBufferSourceNode.detune` (measured in cents)
- No new dependencies — native browser API
- Works on loaded/buffered audio
- Conversion: 0.5× = -1200 cents, 1.0× = 0 cents, 2.0× = +1200 cents

### Quick Settings UI Layout
- Compact sliders row on the Play (front) page
- Two labeled mini sliders: Speed and Pitch, side by side or stacked
- Match the style of existing Quick Settings controls on the Play page

### Claude's Discretion
- Exact slider step increments (e.g. 0.05 or 0.1 steps)
- Label formatting on Quick Settings (match existing Speed label style)
- Where in the component hierarchy the Pitch state lives (likely alongside Speed state)

</decisions>

<specifics>
## Specific Requirements

1. Pitch control in Playback Settings — placed below the existing Speed control, same visual pattern
2. Pitch is purely client-side: adjust `detune` on the Web Audio node when pitch changes
3. Quick Settings on Play page must expose both Speed and Pitch as compact sliders
4. No backend/engine changes for pitch

</specifics>
