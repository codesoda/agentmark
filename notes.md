help me ideate on a new tool idea. It's called agentmark. It's in the vain of agenting coding cli tools being the backbone of many oersonal organization/productivity agent setups lately.

The general idea is that folks have a local folder structure setup and it comprised of markdown, json, jsonl, toml, yaml files and the users have a number of skills/cli tools, alongside desktop tools (obsidian is popular) which can be used to manipulate the underlying files.

Some references
- https://www.chatprd.ai/how-i-ai/teresa-torres-claude-code-obsdian-task-management
- https://www.producttalk.org/give-claude-code-a-memory/
- https://www.lennysnewsletter.com/p/this-week-on-how-i-ai-claude-code (and the multiple referenced/linked articles that expand on this topic)

So the general idea is there is a number of folders with relevant things in them. Coincidentally I was a pocket user. Pocket is/was a bookmarking tool, I could click the pocket icon in the chrome extension and that saved the url into my pocket account. It also showed a small chrome extension window which allowed me to add extra tags to the saved url.

Internally in pocket, it fetched the page, used something like readbility to extract the main article content, very nice. It also (because it had such a large customer base and had the data to correlate) recommended other articles I hadn't viewed or saved.

I want to re-create that experience, but with an agent first angle, and I want to call it AgentMark. Agent + Bookmark, agent first bookmarking for claude code and codex.

- chrome extension: let's me save a url, add optional tags, and ideally even a very short/simple "action", a short message that can be used to instruct an agent on how to save it, or what to do with it (I have other skills, I can hint at their usage with/against this url at capture time).

- agemtmark cli app & agent skill: a simple rust cli, which gets called from the installed chrome extension (chrome extensions can invoke a cli as an action from the extension), this would pass the data through to be saved. If it can be bi-directional then it would also read a list of used tags (added by the user and also suggested ones based on past saved articles). This would have a config which says where the "bookmarks folder is" and then in that folder a markdown file would be created with some front matter, with url, and ideally some other details pulled from og tags, title, description, image(?), publish date( if possible) (we should also add the "bookmarked/saved date"). I would also like to save an agent generated summary, full readbility style article content as well. The user provided tags should be saved to the front matter, and the ai agent should be able to suggest any others as well.

If an "action" is provided then the cli can launch an agent `claude "details go here"` or `codex "details go here"`. config would have configuration about which agent to use for the summary/tagging etc and also to launch actions in.

What other capabilities am I not thinking of? Let brainstorm a PRD for this ai first tool.

---

Yes — this has a real wedge.

What your references all point toward is a broader shift: people are replacing SaaS task/research tools with local markdown- and filesystem-based setups because they want portability, persistent context, and direct agent access. In the examples you linked, that shows up as VS Code/Obsidian workflows, project-specific markdown context, slash commands, and reusable skills instead of traditional dashboards.  ￼

The strongest version of AgentMark is not “Pocket, but with AI summaries.” It is:

a local-first capture bus for personal agent systems

You save a URL once, and AgentMark turns it into a durable, agent-readable object on disk that humans, Claude Code, Codex, Obsidian, and your other tools can all operate on.

One important implementation note: a Chrome extension does not really “just run a CLI directly” in the casual sense. The standard pattern is native messaging, where the extension talks to a registered native host process over stdin/stdout. That is workable, but it will likely be your biggest onboarding tax. Separately, Chrome’s Side Panel API gives you a persistent UI that can stay open across tabs and still use Chrome APIs, which feels better than trying to cram everything into a tiny popup.  ￼

Product thesis

AgentMark helps agent-native users capture web content into their local knowledge filesystem, enrich it automatically, and trigger downstream agent workflows at save time.

A simpler tagline:

AgentMark = agent-first bookmarking for local AI workflows

A sharper one:

Pocket for the Claude Code / Codex era

The key product insight

Treat each saved URL as a durable content bundle, not just a bookmark.

That bundle should have:
	•	the original URL
	•	canonical URL
	•	metadata snapshot
	•	extracted article content
	•	human-facing summary
	•	agent-facing structured fields
	•	tags and collections
	•	action prompt
	•	processing history
	•	links to related local files

That gives you three layers:
	1.	Capture — save the thing fast.
	2.	Enrich — extract, summarize, tag, classify, relate.
	3.	Act — hand it to an agent or a deterministic skill.

That separation matters because it keeps the product composable.

Capabilities you’re probably not thinking about yet

1. Capture more than “the URL”

At save time, also capture:
	•	selected text on the page
	•	page title at capture time
	•	tab group / window context
	•	referrer page
	•	favicon / hero image
	•	whether the user was signed in
	•	reading progress if available
	•	quick note: “why I saved this”

That “why I saved this” field is gold. It often matters more than the summary.

2. Make collections/projects first-class, not just tags

Tags are loose. Agents often need stronger routing.

Examples:
	•	product-discovery
	•	startup-ideas
	•	writing-queue
	•	research/customer-interviews

Collections can map directly to local folders, Obsidian vault areas, or agent command presets.

3. Separate user tags from agent tags

Do not blend them.

Use:
	•	user_tags
	•	suggested_tags
	•	accepted_tags
	•	maybe rejected_tags

That gives better UX and better learning over time.

4. Keep raw and derived artifacts separate

Do not make the markdown note the only artifact.

Preserve:
	•	raw HTML or a fetch snapshot
	•	extracted article markdown/text
	•	metadata JSON
	•	agent-produced outputs

That makes reprocessing possible when your parser, prompts, or models improve.

5. Build a local event log

Every bookmark should have history:
	•	saved
	•	fetched
	•	extraction succeeded/failed
	•	tags suggested
	•	summary regenerated
	•	action dispatched
	•	action completed

This enables debugging, trust, and future automation.

6. Add confidence scores

For publish date, author, summary quality, extracted content quality, and tag relevance.

When confidence is low, route to a review queue instead of pretending certainty.

7. Make “action” a real protocol, not just a freeform string

You’ll want both:
	•	action_prompt: freeform text from the user
	•	action_template: a named workflow like summarize, extract_claims, compare_to_notes, turn_into_tasks, file_under_project

That turns AgentMark from a capture tool into an agent router.

8. Add relatedness early

Not global recommendations like Pocket had. Personal ones.

Examples:
	•	“related to 4 things you already saved”
	•	“contradicts a note in /product/pricing”
	•	“similar to article you saved 3 months ago”
	•	“same domain / same author / same topic cluster”

This will feel magical faster than trying to recreate Pocket’s cross-user recommendation system.

9. Support resurfacing

The product gets much stronger when it brings things back:
	•	weekly digest of unprocessed saves
	•	“you saved this, but never acted on it”
	•	“three new bookmarks match your product-discovery collection”
	•	“this old save is relevant to the page you’re on now”

10. Support migration

This is a big one for your wedge.

Import from:
	•	Pocket export
	•	Raindrop
	•	browser bookmarks
	•	Readwise Reader exports
	•	Instapaper

“Bring your old internet memory into an agent-native filesystem” is a compelling onboarding story.

11. Support site adapters

Generic extraction is fine for v1, but a few site-specific adapters will massively improve quality:
	•	newsletters / Substack
	•	YouTube
	•	GitHub READMEs
	•	docs sites
	•	PDFs
	•	tweet/thread-style pages
	•	academic papers

Mozilla’s Readability is a sensible baseline because it’s available as a standalone version of the library used for Firefox Reader View, but you’ll still want adapters for important page types.  ￼

12. Plan for idempotency and dedupe

Users will save the same thing repeatedly.

You need:
	•	canonical URL normalization
	•	content hash
	•	duplicate detection
	•	versioning when content changed

The right behavior is often: “already saved; add your new note/action to the existing bundle.”

Draft PRD

1. Problem

People using Claude Code, Codex, Obsidian, and local markdown systems can save web links, but they lack a great capture-to-agent workflow. Traditional bookmarking tools optimize for read-later and discovery; modern local-agent users need saved content to become structured, reusable, actionable filesystem objects.

2. Target user

Primary:
	•	agent-native knowledge workers
	•	PMs, founders, researchers, writers, indie hackers
	•	people already using local folders, markdown, Obsidian, Claude Code, Codex, scripts

Secondary:
	•	ex-Pocket / Readwise / Raindrop users who want local ownership
	•	developers building personal workflows on top of files

3. Jobs to be done

When I see something useful on the web:
	•	save it in one click
	•	add a few tags and a note
	•	optionally tell an agent what to do with it
	•	have it land in my local system in a durable, readable structure
	•	resurface it later when relevant
	•	use it as context for future agent work without redoing the setup

4. Core user promise

Save once. Reuse anywhere. Act immediately.

5. MVP scope

Capture
	•	Chrome extension
	•	toolbar click / context menu / keyboard shortcut
	•	fields: tags, collection, quick note, optional action

Ingestion
	•	native host or local helper
	•	fetch page
	•	extract metadata
	•	canonicalize URL
	•	extract readable main content
	•	store raw + cleaned content

Storage
	•	create local bundle on disk
	•	append to global index file
	•	write agent-readable front matter

Enrichment
	•	generate summary
	•	suggest tags
	•	infer collection
	•	detect related existing bookmarks

Action
	•	optionally dispatch to configured runner:
	•	Claude Code
	•	Codex
	•	custom command

Retrieval
	•	CLI search
	•	CLI open
	•	CLI related
	•	CLI reprocess

6. Non-goals for v1
	•	social bookmarking
	•	cloud sync first
	•	cross-user recommendations
	•	team collaboration
	•	perfect automation
	•	building a full reader app

7. UX flows

Flow A: Quick save

Click icon → Save.
No modal unless needed.
Fastest path.

Flow B: Save with intent

Click icon → tag + note + action.
Example action:
“compare this with my pricing notes”
or
“turn this into 3 tasks in the startup-ideas project”

Flow C: Triage

Open side panel → see recent saves → accept tags, edit summary, run an action.

Flow D: Resurface

CLI or agent asks:
“show me saved items related to onboarding activation”

8. Success metrics

Primary:
	•	saves per active user per week
	•	percent of saves successfully enriched
	•	percent of saves later reopened / resurfaced / acted on

Secondary:
	•	average time from click to durable save
	•	duplicate rate handled correctly
	•	percent of agent actions that complete successfully
	•	user acceptance rate of suggested tags

Recommended file model

I would strongly avoid “one giant markdown file per bookmark containing everything.”

Use a folder bundle like this:

agentmark/
  index.jsonl
  collections/
  bookmarks/
    2026/
      03/
        11/
          the-future-of-x-am_01HXYZ/
            bookmark.md
            article.md
            metadata.json
            source.html
            events.jsonl
            hero.jpg

And make bookmark.md the human-facing object:

---
id: am_01HXYZ
url: "https://example.com/post"
canonical_url: "https://example.com/post"
title: "The Future of X"
description: "..."
site_name: "Example"
author: "Jane Doe"
published_at: "2026-03-01"
saved_at: "2026-03-11T09:14:00+10:00"
capture_source: "chrome_extension"
user_tags: ["agents", "productivity"]
suggested_tags: ["knowledge-management", "workflow-design"]
collections: ["product-discovery"]
note: "Might be useful for our research workflow"
action_prompt: "compare this to my existing capture system"
state: "inbox"
content_status: "extracted"
summary_status: "done"
schema_version: 1
---

# Summary

...

# Why I saved this

...

# Suggested next actions

...

# Related items

...

That structure gives you:
	•	human readability
	•	agent readability
	•	reprocessing safety
	•	compatibility with Obsidian and plain git

Architecture suggestion

Think in terms of pipelines, not one monolithic command.

1. Extension layer
	•	popup for fast save
	•	side panel for triage
	•	context-menu save for selected text
	•	maybe keyboard shortcut

2. Transport layer
	•	native messaging host or local daemon

3. Ingestion layer
	•	fetch
	•	parse
	•	canonicalize
	•	extract readable content
	•	store assets

4. Enrichment layer
	•	summarizer
	•	tagger
	•	classifier
	•	similarity engine
	•	quality scorer

5. Action layer
	•	runner abstraction:
	•	claude
	•	codex
	•	custom shell command

6. Query layer
	•	local index
	•	BM25 / sqlite FTS first
	•	embeddings later if needed

My bias: use a simple local sqlite DB or index.jsonl + sqlite FTS for retrieval, even if files remain the source of truth. Pure filesystem scanning will get annoying faster than people expect.

Best “agent-first” features

These feel especially on-theme for AgentMark:
	•	Save and ask: save page + run action immediately
	•	Save as context: route to a project folder and update its context index
	•	Save and synthesize: compare with related saved items
	•	Save and taskify: turn article into next actions
	•	Save and decompose: extract claims, questions, decision points
	•	Save with skill hint: “use research-digest skill on this”
	•	Save selection only: useful for pages where full-page extraction is noisy

Where the moat actually is

Not in summary quality.

Your moat is:
	•	the local schema
	•	the capture ergonomics
	•	the bridge into agent workflows
	•	the personal recommendation / resurfacing layer
	•	the interoperability with existing filesystem habits

Anyone can call an LLM to summarize a URL.
Much fewer tools become a durable part of someone’s personal operating system.

Biggest risks

1. Onboarding friction

Native host install and filesystem config can scare off non-technical users.

2. Extraction brittleness

Some sites will parse beautifully; others will be a mess.

3. Agent unpredictability

Freeform action prompts are powerful but can feel unreliable.
You’ll want templates and guardrails.

4. Over-automation

If everything gets auto-tagged, auto-filed, and auto-summarized, people may stop trusting the system.

5. Too much in one object

If bookmark files become giant dumping grounds, the system turns into sludge.

My product advice

For v1, focus hard on three things:
	1.	best-in-class capture speed
	2.	clean local artifact generation
	3.	one delightful save-to-action workflow

Do not chase Pocket-style recommendations first.
Do not try to build a whole reader.
Do not overfit to one agent.

If AgentMark nails “save this URL into my local agent system in a way that remains useful for months,” you already have something pretty differentiated.

A strong next step would be turning this into a 1-page PRD with explicit MVP requirements, CLI commands, and a first-pass folder/schema spec.
