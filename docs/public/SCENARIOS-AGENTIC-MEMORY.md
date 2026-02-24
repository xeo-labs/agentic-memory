# Capability Scenarios: AgenticMemory

*What happens when an AI agent can remember, reason about its reasoning, and correct itself across sessions?*

---

## CAUSED_BY Edges — The Agent Can Walk Backwards Through Its Own Thinking

You're three sessions deep into a migration from PostgreSQL to CockroachDB. The agent recommended CockroachDB in session one. Now it's session four, and latency numbers are worse than expected. You ask: "Why did you recommend CockroachDB in the first place?"

Without CAUSED_BY edges, the agent stares blankly. It doesn't remember session one. It doesn't know what facts led to that decision. It might hallucinate a justification, or worse, confidently restate the recommendation without acknowledging the latency problem. You're left forensically reading chat logs, trying to reconstruct a reasoning chain that no longer exists in any machine-readable form.

With CAUSED_BY edges, the agent traverses its own reasoning graph backwards. It finds the Decision node "Chose CockroachDB for multi-region deployment" and follows three CAUSED_BY edges to the supporting facts: "Team requires sub-100ms reads across US-East and EU-West," "CockroachDB supports follower reads," and "PostgreSQL Citus extension lacks transparent geo-routing." The traversal takes 3.4 milliseconds across 847 nodes. The agent responds:

> I recommended CockroachDB because of three facts I recorded in session 1: your team needed sub-100ms multi-region reads, CockroachDB supports follower reads natively, and Citus lacked transparent geo-routing. Given the latency regression you're seeing, the second fact may need revisiting — follower reads have consistency trade-offs I didn't weight heavily enough.

The agent doesn't just remember what it decided. It remembers *why*, and it can critically re-examine the chain when new evidence arrives.

**In plain terms:** CAUSED_BY edges give the agent a breadcrumb trail through its own thinking. Instead of a goldfish that forgets between conversations, you get a colleague who can explain every recommendation back to the evidence that motivated it.

---

## SUPERSEDES + RESOLVE — Facts That Update Other Facts

Your team's primary language was Python when the agent first learned about the project in January. By March, you've migrated critical services to Rust. You never explicitly told the agent to "forget Python" — you just started talking about Rust codebases and cargo build times.

Without supersedes chains, the agent holds contradictory beliefs simultaneously. It might suggest a Python testing framework for your Rust service. It might recommend Flask middleware when you need Actix extractors. Every old fact persists with equal authority, and the agent has no mechanism to distinguish "currently true" from "was once true." You catch the inconsistencies manually, correcting the agent repeatedly, burning context window on information it should already have.

With SUPERSEDES and RESOLVE, when the agent learns "Primary language is now Rust," it creates a Correction node that supersedes the original Fact "Primary language is Python." The old fact isn't deleted — it's still there, timestamped, auditable. But when any query touches "primary language," the resolve chain follows the supersedes edge in under 1 millisecond and returns the canonical current answer: Rust. The history is preserved. The correction is automatic. Three sessions later, you can even ask "What did you used to think our primary language was?" and get a precise answer with timestamps.

> Your primary language is Rust (corrected in session 15). Previously, I recorded Python as your primary language in session 1. The correction was triggered when you described migrating payment-service and auth-service to Rust.

**In plain terms:** SUPERSEDES chains are version control for beliefs. The agent never loses history, but it always gives you the latest truth — like a wiki page that tracks every edit but shows the current version by default.

---

## Six Cognitive Event Types — Typed Thinking, Not a Blob of Text

You ask the agent to analyze a production incident. It reads logs, makes inferences, forms a hypothesis, decides on a remediation plan, and documents a reusable runbook.

Without typed events, all of this reasoning gets stored (if it's stored at all) as undifferentiated text blobs. The agent can't distinguish between "something I observed" and "something I concluded" and "something I decided." When you later ask "What decisions did you make about the incident?", the agent would need to re-read and re-classify every piece of stored information. Facts, guesses, and conclusions blur together. The agent's confidence in a hunch looks identical to its confidence in a measured observation.

With six cognitive event types — Fact, Decision, Inference, Correction, Skill, Episode — each piece of reasoning gets a type tag at creation. The production incident analysis produces: 3 Facts ("CPU spiked to 94% at 14:32 UTC," "Connection pool maxed at 200," "No deployments in the last 6 hours"), 2 Inferences ("Likely a slow query cascade, not a code regression"), 1 Decision ("Increase connection pool to 400 and add query timeout of 30s"), and 1 Skill ("When CPU spikes without deployment, check slow query log before scaling horizontally"). Later, `memory_query` with `event_types: ["decision"]` instantly returns only the decisions, no re-classification needed. A pattern query for all Skills gives you a growing library of reusable runbooks.

> Found 1 decision related to the March 12 incident: "Increase connection pool to 400, add 30s query timeout." Supporting inferences: slow query cascade (confidence 0.85). Want me to show the full reasoning chain?

**In plain terms:** Typed events turn a junk drawer into a filing cabinet. The agent doesn't just remember things — it remembers *what kind* of thing each memory is, so it can retrieve decisions without wading through observations, and separate facts from guesses.

---

## Cross-Session Persistence — Memory That Survives Conversation Boundaries

You're building a complex microservices architecture over six weeks. Each conversation is a new context window. Each new session starts with zero knowledge of the previous ones.

Without cross-session persistence, every Monday morning is Groundhog Day. You re-explain the architecture. You re-state your preferences. You re-describe the database schema. The agent asks you the same clarifying questions it asked last Thursday. By the third week, you've spent more time re-briefing the agent than actually building. The agent's maximum useful memory is one conversation, regardless of how long the project runs.

With cross-session persistence via the `.amem` binary format, the agent opens last session's memory file in 3.7 milliseconds (10,000 nodes) and has instant access to every fact, decision, inference, and correction from all prior sessions. Session 12 knows what session 1 established. The agent picks up exactly where it left off. It remembers that you prefer explicit error handling over Result-wrapping, that the payment service talks to Stripe via webhook, and that the team decided against GraphQL in session 4 (with a CAUSED_BY chain explaining why).

> Resuming from session 11 (last Tuesday). Your architecture has 7 services. We left off designing the notification service. You preferred webhook delivery over polling. The payment-service Stripe integration is still pending the idempotency key refactor we discussed.

Storage is 8 MB per 10,000 nodes. A decade of daily agent use fits in 240 MB. Your agent's lifetime memory costs less than a single high-resolution photo.

**In plain terms:** Cross-session persistence turns a series of disconnected conversations into a continuous working relationship. The agent remembers you the way a long-term colleague does — not just what you said today, but the full context of your collaboration.

---

## Semantic Search — Finding by Meaning, Not Keywords

You remember discussing a "scaling bottleneck" in the database layer three weeks ago, but you can't remember the exact words you used. Was it "scaling bottleneck"? "Performance ceiling"? "Throughput limit"?

Without semantic search, you'd need to guess the exact keywords. A text search for "scaling bottleneck" misses the node where you said "the database can't handle more than 10K writes per second." A search for "throughput" misses the node where the agent noted "PostgreSQL connection pool saturating under load." The information exists in memory, but keyword mismatch makes it invisible. You end up re-explaining the problem, and the agent generates a new analysis that contradicts its own prior work.

With semantic search over 128-dimensional feature vectors, the query "database scaling problems" finds all semantically related nodes in 9 milliseconds across 100,000 nodes. The agent returns the original fact about 10K write limits, the inference about connection pool saturation, the decision to evaluate CockroachDB, and the subsequent correction when latency numbers came back worse than expected. Cosine similarity doesn't care about word choice — it matches meaning.

> Found 4 memories related to "database scaling problems": a fact about the 10K write/s ceiling (session 3, confidence 0.92), an inference about connection pool saturation (session 5, confidence 0.78), a decision to evaluate CockroachDB (session 7), and a correction noting higher-than-expected latency (session 9). Want me to trace the full reasoning chain?

**In plain terms:** Semantic search is like asking a colleague "remember when we talked about the database not keeping up?" and having them immediately recall the conversation, even though you used completely different words the first time around.

---

## Hybrid Search — Combining Semantic and Structural Retrieval

You're looking for "the Stripe webhook handler decision." You know the exact term "Stripe" and the general concept of "webhook processing decisions."

Without hybrid search, you choose between precision and recall. A keyword search for "Stripe" returns 47 nodes — every time Stripe was mentioned, including tangential references. A semantic search for "webhook processing decisions" might miss the specific Stripe context because the embedding space doesn't perfectly preserve proper nouns. Neither approach alone gives you the needle in the haystack.

With hybrid search using Reciprocal Rank Fusion, both BM25 text matching and vector similarity run simultaneously and their rankings merge. BM25 ensures "Stripe" appears in the results. Vector similarity ensures "webhook handler decision" semantics are weighted. The fusion takes 10.83 milliseconds and returns the exact Decision node: "Implement Stripe webhook handler with idempotency keys and 3-retry exponential backoff." It was ranked 12th by BM25 alone and 8th by vector alone, but RRF fusion pushed it to rank 1.

> Top result (hybrid score 0.91): Decision from session 8 — "Implement Stripe webhook handler with idempotency keys and 3-retry exponential backoff." BM25 matched on "Stripe," vector matched on "webhook processing decision pattern."

**In plain terms:** Hybrid search is like having two search strategies — one that reads exact words, one that understands meaning — and combining their best guesses. You get the precision of keywords with the flexibility of meaning-based retrieval.

---

## Temporal Queries — "What Did I Know at Time T?"

A production incident happened on February 15th. You need to understand what the agent believed about the system *at that time*, not what it believes now (after three corrections and a major refactor).

Without temporal queries, the agent can only show you the current state of knowledge. If a fact was corrected on February 20th, the original belief is buried in a supersedes chain that you'd need to manually traverse. You can't ask "before the incident, what did we think about connection pooling?" because the agent's view of "connection pooling" has been updated since then. Post-hoc rationalization becomes indistinguishable from pre-incident understanding.

With temporal queries comparing two time windows, the agent isolates the exact knowledge state at any moment. `memory_temporal` comparing February 1-14 against February 15-28 shows: 3 facts that were added after the incident, 2 corrections applied to pre-incident beliefs, and 1 decision that was unchanged. The pre-incident state is fully reconstructable — the agent believed the connection pool was sized correctly (confidence 0.88), didn't know about the slow query regression, and had a Skill node for "standard incident response" that didn't include database analysis.

> At February 15th, your memory contained: 142 nodes across 8 sessions. Key belief: connection pool at 200 was adequate (confidence 0.88). This was corrected on February 20th to "connection pool needs 400 minimum." The correction was CAUSED_BY the incident finding.

**In plain terms:** Temporal queries give you a time machine for beliefs. You can rewind to any date and see exactly what the agent knew, didn't know, and was wrong about — which is invaluable for incident postmortems and understanding how knowledge evolves.

---

## Pattern Queries — "Find All Decisions About X"

You're onboarding a new team member and want to show them every architectural decision the agent has recorded about authentication over the past 30 sessions.

Without pattern queries, you'd keyword-search for "auth" and get a mix of facts, inferences, tangential mentions, and actual decisions. Then you'd manually filter for just the decision-type entries, sort them chronologically, and try to reconstruct the decision narrative. This might take 20 minutes of sifting through results.

With pattern queries, `memory_query` with `event_types: ["decision"], sort_by: "most_recent"` combined with semantic similarity to "authentication" returns exactly what you need in 40 milliseconds: 7 Decision nodes across 30 sessions, chronologically ordered, each with its confidence score and CAUSED_BY links to supporting facts. Session 3: "Use JWT for service-to-service auth." Session 9: "Add refresh token rotation." Session 14: "Migrate to OAuth 2.1 for user-facing endpoints." Session 22: "Add rate limiting to auth endpoints after brute-force attempt."

> Found 7 authentication decisions across sessions 3-28. The narrative arc: JWT for internal auth → refresh tokens → OAuth 2.1 for users → rate limiting post-incident. Each decision links to 2-4 supporting facts. Want me to generate a decision log document?

**In plain terms:** Pattern queries are like asking "show me every time we made a call about authentication" and getting a clean, typed, chronological decision log — no noise, no manual filtering, just the decisions and their reasoning chains.

---

## Causal Analysis — "What Depends on This Fact?"

You just discovered that the latency numbers from your benchmarking tool were wrong. The tool had a configuration error that inflated all measurements by 40%. You need to know: what decisions were made based on those bad numbers?

Without causal analysis, you'd have to mentally trace every place latency was discussed. Did the agent use those numbers to recommend a database? To size a cache? To reject a particular framework? The blast radius of a bad fact is invisible. You might correct the latency numbers but leave downstream decisions uncorrected — a silent knowledge corruption that compounds over time.

With `memory_causal`, the agent traverses all downstream edges from the bad latency fact and returns the full dependency tree in 30 milliseconds. Result: 2 Inferences that used the latency fact as evidence ("Service X is too slow for real-time" — now wrong), 1 Decision that followed from those Inferences ("Add a caching layer between Service X and the API gateway" — now questionable), and 3 further nodes that depend on the caching decision. Total blast radius: 6 nodes, 2 of which are high-confidence Decisions that need re-evaluation.

> If the latency benchmarks were wrong, 6 downstream memories are affected: 2 inferences (Service X performance assessment), 1 decision (caching layer addition), and 3 dependent design choices. The caching layer decision has the highest impact — 3 other nodes depend on it. Recommend re-running benchmarks and then using `memory_correct` to update the chain.

**In plain terms:** Causal analysis is a blast radius calculator for beliefs. When you discover a fact was wrong, the agent instantly shows you everything that was built on that bad foundation — like finding a cracked brick and knowing exactly which walls are affected.

---

## Memory Quality Scoring — Confidence and Reliability Metrics

Your agent has been running for 6 months. It has 12,000 nodes across 180 sessions. How healthy is its knowledge base? Are there stale beliefs nobody has challenged? Decisions that were never backed by evidence?

Without quality scoring, memory is a black box. You trust it until something goes wrong. A low-confidence inference from session 2 carries the same weight as a well-established fact from session 150. Decisions based on guesses look identical to decisions based on rigorous analysis. Knowledge debt accumulates silently.

With `memory_quality`, the agent runs a comprehensive health audit: 23 nodes with confidence below 0.45, 7 orphan nodes (no edges connecting them to anything), 4 decisions without any supporting CAUSED_BY or Supports edges, and 31 stale nodes that haven't been accessed in 60 days. The health score is 0.87 out of 1.0. The agent reports the 4 unsupported decisions as the highest-priority concern — decisions floating without evidence are the most dangerous form of knowledge debt.

> Memory health audit: 12,847 nodes, 31,204 edges. Health score: 0.87. Concerns: 4 decisions lack supporting evidence (highest risk), 23 low-confidence nodes, 31 stale nodes. The unsupported decisions are about caching strategy (session 12), retry policy (session 34), timeout values (session 41), and rate limiting (session 67). Recommend adding supporting facts or re-evaluating these decisions.

**In plain terms:** Memory quality scoring is a health checkup for the agent's brain. Like a code linter catches technical debt, quality scoring catches knowledge debt — beliefs that are stale, unsupported, or dangerously low-confidence.

---

## Contradiction Detection — Finding Conflicting Facts

Over 50 sessions, different team members have given the agent conflicting information. Alice said the API rate limit is 1,000 requests per minute. Bob said it's 500. The agent recorded both as Facts without noticing they contradict each other.

Without contradiction detection, both facts coexist peacefully. The agent might use either one depending on which retrieval path happens to activate first. One day it tells you the rate limit is 1,000; the next day, 500. The inconsistency is invisible until it causes a real problem — like building a load balancer that assumes 1,000 when the actual limit is 500.

With contradiction detection via Contradicts edges and the consolidation query's `LinkContradictions` operation, the agent identifies 3 contradiction pairs across the entire graph in 25 milliseconds. The rate limit contradiction is flagged with a contradiction strength of 0.89. The `reasoning_gaps` query then reports it as an `UnstableKnowledge` gap with severity 0.85. The agent surfaces the conflict proactively:

> Found contradiction: "API rate limit is 1,000 req/min" (session 12, from Alice, confidence 0.80) conflicts with "API rate limit is 500 req/min" (session 34, from Bob, confidence 0.75). These can't both be true. Should I verify with the API documentation and record a correction?

**In plain terms:** Contradiction detection is an internal consistency checker. When the agent holds two beliefs that can't both be true, it catches the conflict before it causes problems — like a compiler that won't let you declare the same variable with two different types.

---

## Budget Policy — Long-Horizon Storage Governance

Your agent has been running for 2 years. At 8 MB per 10,000 nodes, the memory file is approaching 150 MB. You want the agent to keep running for another 18 years without manual storage maintenance.

Without budget governance, the memory file grows without bound. Every fact, every inference, every tangential observation accumulates forever. Eventually, you hit disk limits or performance degrades from sheer size. Manual pruning requires understanding which nodes are safe to remove — a task that demands expertise in the agent's own knowledge graph.

With storage budget policy (`AMEM_STORAGE_BUDGET_MODE=auto-rollup`, `AMEM_STORAGE_BUDGET_BYTES=2147483648`), the agent projects its growth over the configured 20-year horizon. When the file reaches 85% of the 2 GB budget, the consolidation engine activates automatically: near-duplicate facts merge via deduplification (0.95 similarity threshold), stale Episode nodes from completed sessions compress into summary nodes, and low-decay orphan nodes are flagged for archival. The agent keeps every high-value Decision and Fact while compressing routine observations. The 2 GB budget accommodates roughly 2.5 million nodes — a lifetime of continuous use.

> Storage health: 147 MB / 2 GB (7.3%). Projected 20-year usage: 890 MB. Budget status: healthy. Auto-rollup has compressed 312 low-value episode nodes this quarter. All decisions and corrections preserved.

**In plain terms:** Budget policy is a pension plan for memory. The agent manages its own storage lifecycle so it can run for decades without human intervention — compressing the mundane while preserving every important decision.

---

## Privacy & Redaction Controls — AMEM_AUTO_CAPTURE

Your agent captures decisions and facts automatically during conversations. But some of those conversations contain API keys, email addresses, customer names, and internal URLs that should never be persisted.

Without privacy controls, the agent stores whatever it encounters. An API key mentioned in passing becomes a permanent fixture of the memory graph. An email address from a support ticket gets embedded in a Fact node. You discover this months later during an audit and realize the memory file has been synced to a shared drive.

With `AMEM_AUTO_CAPTURE_MODE=safe` and `AMEM_AUTO_CAPTURE_REDACT=true`, the agent applies regex-based redaction before any content is persisted. Email patterns become `[redacted-email]`. API keys matching `sk-*` become `[redacted-secret]`. Local filesystem paths become `[redacted-path]`. The `AMEM_AUTO_CAPTURE_MAX_CHARS=2048` setting truncates excessively long captures. The `safe` mode only captures structured reasoning (decisions, facts, corrections) and skips raw conversation content entirely.

> Captured 8 nodes this session. Redaction applied: 2 email addresses removed, 1 API key removed. Mode: safe (structured reasoning only). Raw conversation content: not captured.

**In plain terms:** Privacy controls are a firewall between conversation content and persistent memory. The agent remembers the reasoning and decisions, but sensitive details like API keys and email addresses are scrubbed at the gate — privacy by construction, not by policy.

---

## Runtime-Sync Episodes — Handoff Snapshots Between Sessions

You have three agents working on different parts of the same project. Agent A designed the database schema. Agent B is building the API. Agent C is writing tests. Each has its own memory file. Agent B needs to know what Agent A decided about the schema without re-reading every conversation.

Without runtime-sync, each agent is an island. Agent B would need you to manually summarize Agent A's decisions and paste them in. Context transfer is manual, lossy, and time-consuming. Important decisions fall through the cracks. Agent C writes tests against an outdated understanding of the API.

With `amem runtime-sync`, Agent A's memory file is scanned and a compressed Episode node is created summarizing the key decisions, facts, and corrections. This episode becomes a handoff artifact that Agent B can ingest. The Episode contains: 12 Decisions about table design, 3 Corrections where the schema was revised, and 5 key Facts about foreign key constraints. Agent B now has Agent A's architectural knowledge without the noise of every intermediate inference and tangential discussion.

> Runtime-sync episode created: "Database Schema Design (sessions 1-15)." Contains 12 decisions, 3 corrections, 5 key facts. Workspace changes detected: 4 new migration files, 2 modified models. Episode ready for cross-agent handoff.

**In plain terms:** Runtime-sync episodes are briefing documents that agents write for each other. Instead of making Agent B read Agent A's entire diary, you get a curated summary of the important decisions — like a shift handoff report in a hospital.

---

## All Together Now: Multi-Session Incident Response

It's Tuesday. Your production monitoring triggers an alert: the payment service is returning 500 errors at a rate of 12% of all requests. You engage the agent.

**Session 1: Triage (10:00 AM)**

The agent starts a session and records the initial facts: "Payment service 500 error rate: 12%", "Alert triggered at 09:47 UTC", "No deployments in the last 24 hours." These are Fact nodes with confidence 0.95 (direct observations). The agent creates an Inference: "Likely not a code regression — no recent deploys" (confidence 0.72, CAUSED_BY the "no deployments" fact). It checks memory from previous sessions and finds a Skill node from 3 months ago: "When payment service errors spike without deployment, check Stripe API status first." Following that Skill, it checks Stripe status and records a new Fact: "Stripe API status: operational, no incidents."

The Inference is updated. The agent creates a second Inference: "If not code regression and not Stripe, likely database or connection issue" (confidence 0.65, CAUSED_BY the first inference and the Stripe status fact). It creates a Decision: "Investigate database connection pool next" (CAUSED_BY the second inference).

**Session 2: Investigation (11:30 AM)**

The agent loads the memory from session 1 in 3.7 milliseconds. It sees the decision to investigate the connection pool and records new facts from the database metrics: "Connection pool utilization: 198/200 (99%)", "Average query time: 340ms (normal: 45ms)", "Slow query log shows 3 queries > 5 seconds."

It creates an Inference: "Connection pool exhaustion caused by slow queries, which are backing up all requests" (confidence 0.88). This inference has three CAUSED_BY edges pointing to the three facts. The agent also creates a Correction: "Previous inference 'likely database or connection issue' is confirmed — specifically connection pool exhaustion from slow queries." The Correction node SUPERSEDES the vaguer inference from session 1.

The agent decides: "Add query timeout of 30 seconds, increase pool to 400, and identify the slow queries for optimization." The decision has CAUSED_BY edges to the confirmed inference.

**Session 3: Remediation (2:00 PM)**

Memory loads again. The agent sees the remediation decision and begins execution. It records the outcome as new Facts: "Pool increased to 400 — error rate dropped to 2%", "Query timeout at 30s — no more pool exhaustion", "Identified 3 slow queries: order history aggregation, refund calculation, and monthly report." Error rate is down but not zero.

The agent creates a new Inference: "The 2% residual errors are likely from the slow queries hitting the 30-second timeout and returning errors to clients" (confidence 0.80). It creates a Decision: "Optimize the 3 slow queries rather than just timing them out." For the "order history aggregation" query, it finds a semantically similar Skill node from 2 months ago: "Aggregation queries on large tables benefit from materialized views" (found via `memory_similar` in 9 milliseconds, similarity: 0.87).

**Session 4: Postmortem (Wednesday, 10:00 AM)**

The agent loads all 4 sessions of incident memory. You ask it to generate a postmortem. The agent uses `memory_query` to find all Decisions (4 found), all Corrections (1 found), and all Facts (11 found). It uses `memory_causal` on the initial "12% error rate" fact and traces the complete dependency tree: 11 facts → 4 inferences → 3 corrections → 4 decisions → 1 skill applied.

Then you ask: "What if we had caught the slow queries earlier?" The agent runs `belief_revision` with the hypothetical "Slow query monitoring was in place." It identifies 2 decisions that would have been unnecessary (emergency pool increase, timeout addition) and 1 inference that wouldn't have been needed (the connection exhaustion analysis). Estimated time savings: the first two sessions could have been collapsed into 30 minutes.

The agent records the entire postmortem as an Episode node, with a Skill node: "Add slow query alerting to payment service monitoring — prevents connection pool cascades." This Skill is now available for future incidents.

> Postmortem complete. Root cause chain: 3 slow queries → connection pool exhaustion → 12% error rate. Resolution: pool increase (immediate), query timeout (mitigation), query optimization (permanent fix). New skill recorded: slow query alerting prevents cascade. Belief revision shows this could have been a 30-minute fix with proper monitoring.

Four sessions. One incident. Seventeen memory nodes. Twenty-three edges. Every decision traceable to evidence, every correction preserving history, every skill reusable in future incidents. The agent didn't just solve the problem — it built institutional knowledge that makes the next incident faster.

**In plain terms:** This is the difference between an amnesiac firefighter who forgets each blaze and a seasoned veteran who remembers every incident, knows which patterns repeat, and gets faster with every call. AgenticMemory turns throwaway conversations into compounding institutional intelligence.
