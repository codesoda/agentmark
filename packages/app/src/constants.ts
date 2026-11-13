// Install and listing constants (docs/setup-contract.md §7.1).
// An empty string means "does not exist yet": the UI hides whatever uses it.

/** The Chrome Web Store listing. Empty: the app shows the development steps instead. */
export const WEB_STORE_URL: string = "";

/** The public website. */
export const SITE_URL = "https://agentmark.app";

/** The source repository. */
export const REPO_URL = "https://github.com/codesoda/agentmark";

/** How often the "Add the Chrome extension" screen re-reads the liveness file (contract §9). */
export const CONNECTION_POLL_MS = 1000;
