/**
 * Options for a select whose saved value may predate the prebuilt list.
 * A value missing from `choices` is appended so the native select still shows
 * it selected instead of blank, and the saved id is never silently replaced.
 */
export function choicesWithCurrent(
  choices: readonly string[],
  current: string | null | undefined
): string[] {
  if (!current || choices.includes(current)) return [...choices];
  return [...choices, current];
}
