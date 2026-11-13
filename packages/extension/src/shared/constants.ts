export const NATIVE_HOST_NAME = "com.agentmark.native";
export const EXTENSION_NAME = "AgentMark";

// --- Install and listing constants (docs/setup-contract.md §7.1) ---
// An empty string means "does not exist yet": the UI hides whatever uses it.

/**
 * The Chrome Web Store ID. Must equal the CLI's `setup::extension_ids` copy and
 * the ID the manifest `key` gives (both checked by tests).
 */
export const WEB_STORE_EXTENSION_ID = "oepdfndmpcpimbkaaocgolenncdkljeo";

/** The Chrome Web Store listing. */
export const WEB_STORE_URL = "";

/** Desktop app download for Apple Silicon Macs. */
export const MAC_APP_DOWNLOAD_URL = "";

/** Desktop app download for Intel Macs. */
export const MAC_INTEL_APP_DOWNLOAD_URL = "";

/** The public website. */
export const SITE_URL = "https://agentmark.app";

/** The installer script that the terminal commands pipe into bash. */
export const INSTALL_SCRIPT_URL = "https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh";

/** Public setup guide, opened from the install prompt. */
export const SETUP_GUIDE_URL = `${SITE_URL}/#install`;

// --- Storage keys shared by the popup, side panel and service worker ---

/** `chrome.storage.session`: the popup's unfinished setup answers (contract §4.4). */
export const SETUP_DRAFT_KEY = "agentmark_setup_draft";

/** `chrome.storage.session`: which view the side panel should show when it opens (contract §3.4). */
export const SIDEPANEL_VIEW_KEY = "agentmark_sidepanel_view";

/** Message type a content script sends to report its own page (contract §3.3). */
export const REPORT_PAGE_MESSAGE = "report_page";
