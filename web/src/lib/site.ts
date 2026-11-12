export const SITE_URL = "https://agentmark.app";
export const REPO_URL = "https://github.com/codesoda/agentmark";
export const INSTALL_CMD = "curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash";
export const SITE_TITLE = "AgentMark: bookmarks your agent can read";
export const SITE_DESCRIPTION =
  "AgentMark saves web pages as plain files on your machine, with a local full-text index. Claude Code and Codex can search and read them. No account, no sync, no AgentMark server.";

// --- Install constants (docs/setup-contract.md §7.1) ---
// Names and values match packages/extension/src/shared/constants.ts. An empty
// string means "does not exist yet": the install section then shows the path
// that works today instead of a link. Fill one in and the page switches.

/** The Chrome Web Store listing. */
export const WEB_STORE_URL = "";

/** Desktop app download for Apple Silicon Macs. */
export const MAC_APP_DOWNLOAD_URL = "";

/** Desktop app download for Intel Macs. */
export const MAC_INTEL_APP_DOWNLOAD_URL = "";

/** The installer script that the terminal commands pipe into bash. */
export const INSTALL_SCRIPT_URL = "https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh";
