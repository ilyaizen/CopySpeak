---
phase: quick-5
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/lib/components/hud-overlay.svelte
autonomous: true
requirements: []
must_haves:
  truths:
    - "Dev mode HUD shows waveform with first 2 and last 2 bars visually inactive (at minimum height)"
    - "Active bars (2-13) animate with the multi-wave pattern"
    - "Waveform bars rise quickly and decay slowly (attackRate 0.6, decayRate 0.15)"
  artifacts:
    - path: "src/lib/components/hud-overlay.svelte"
      provides: "Dev mock with correct inactive edge bars and updated attackRate"
  key_links:
    - from: "hud-overlay.svelte dev mock"
      to: "waveform.svelte barValues prop"
      via: "bars array with [0,0,...,0,0] at edges"
      pattern: "bars.push\\(0, 0\\)"
---

<objective>
Fix two small discrepancies in hud-overlay.svelte against the waveform improvements spec:
1. Dev mock pushes `[1, 1]` as the first two bars — should be `[0, 0]` (inactive/excluded bars)
2. Waveform component usage passes `attackRate={0.5}` — spec recommends `0.6` for snappier response

Note: buildBarValues() in playback-store.svelte.ts and all smoothing/decay logic in waveform.svelte are already fully implemented per the spec. Only hud-overlay.svelte needs updating.

Purpose: Ensure the dev mock accurately represents the live behaviour (edge bars silent) and attack rate matches tuned values.
Output: Updated hud-overlay.svelte with correct dev mock and attackRate.
</objective>

<execution_context>
@./.claude/get-shit-done/workflows/execute-plan.md
@./.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md

<interfaces>
<!-- Current hud-overlay.svelte dev mock (lines 185-201) — what needs fixing -->

Dev mode mock block (current — wrong):
```svelte
const animateBars = () => {
  const bars: number[] = [1, 1]; // First two inactive  ← BUG: should be [0, 0]
  for (let i = 0; i < 12; i++) {
    ...
  }
  bars.push(0, 0); // Last two inactive  ← correct
  barValues = bars;
  devAnimationId = requestAnimationFrame(animateBars);
};
```

Waveform usage in hud-overlay.svelte (lines 250-259) — wrong attackRate:
```svelte
<Waveform
  {barValues}
  barColor="rgba(255, 255, 255, 0.3)"
  activeBarColor="rgba(96, 165, 250, 1)"
  barGap={3}
  barRadius={2}
  minBarHeight={0.15}
  attackRate={0.5}   ← should be 0.6
  decayRate={0.15}
/>
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Fix dev mock inactive bars and update attackRate</name>
  <files>src/lib/components/hud-overlay.svelte</files>
  <action>
Make two targeted edits to src/lib/components/hud-overlay.svelte:

1. Line ~186: Change `const bars: number[] = [1, 1];` to `const bars: number[] = [0, 0];`
   - The comment says "First two inactive" but the value 1 makes them active (above minBarHeight 0.15)
   - Must be 0 to match the frequency mapping that excludes edge bars

2. Line ~257: Change `attackRate={0.5}` to `attackRate={0.6}`
   - Per the spec: 0.6 gives snappier visual response when bars rise

Do NOT change anything else — the multi-wave pattern in the mock, the last two bars, decayRate, or any other props are already correct.
  </action>
  <verify>
    <automated>rtk git diff src/lib/components/hud-overlay.svelte</automated>
  </verify>
  <done>
- `[1, 1]` replaced with `[0, 0]` in dev mock initializer
- `attackRate={0.5}` replaced with `attackRate={0.6}` in Waveform usage
- No other lines changed
  </done>
</task>

</tasks>

<verification>
Open the HUD route in dev mode (`bun run dev`, visit http://localhost:5173/hud) and confirm:
- First 2 and last 2 bars render at minimum height (thin dimmed bars), not at full amplitude
- Middle 12 bars animate with the wave pattern
- Bars feel snappy on rise and hold weight on decay
</verification>

<success_criteria>
- Edge bars (0, 1, 14, 15) are visually inactive in the dev mock
- attackRate is 0.6 in the Waveform component call
- `bun run test` passes (no regressions)
</success_criteria>

<output>
After completion, create `.planning/quick/5-implement-hud-waveform-improvements-excl/5-SUMMARY.md`
</output>
