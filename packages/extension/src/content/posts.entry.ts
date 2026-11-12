import { reportPageIfChanged } from "./page-report";
import { watchPosts } from "./save-button";
import { adapterFor } from "./site-adapters";

const adapter = adapterFor(window.location.hostname);
// Each pass also reports the page, so the toolbar icon can show it saved without `tabs`.
if (adapter) watchPosts(document, adapter, () => reportPageIfChanged());
