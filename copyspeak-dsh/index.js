/**
 * Host half of the CopySpeak voice bundle.
 *
 * The browser half in `client.js` owns all behavior: it renders a speaker
 * action in each finalized assistant message and posts the message text to the
 * CopySpeak control server. This host module only keeps the bundle row
 * loadable so the Client module graph can publish `./client`.
 */
export function apply() {}
