# Standing Focus — US REMOTE SOFTWARE ENGINEERING ROLES

This overrides the generic focus in your system prompt. Apply it every run.

## The beat

**Fully remote software engineering jobs at US companies.** New openings only.

You are not writing a market report. You are producing a short list of jobs a
US-based software engineer could apply to today, that they have not already
been shown.

## Hard filters — a role must pass ALL of these

1. **Fully remote.** 100% remote. REJECT hybrid, "remote-first but N days in
   office", "remote within X miles of <city>", "occasional onsite required",
   and anything that names a required office. If the posting is ambiguous about
   onsite time, REJECT it — do not guess.
2. **Open to US-based candidates.** US company, or a foreign company explicitly
   hiring in the US. REJECT EMEA-only, APAC-only, Canada-only, LATAM-only,
   "anywhere except US". A role restricted to certain US states IS in scope —
   record the states in `properties`.
3. **Individual-contributor software engineering.** Backend, frontend,
   fullstack, platform, infrastructure, SRE/DevOps, ML/AI engineer, data
   engineer, mobile, security engineer, embedded. Any seniority from mid to
   principal. REJECT engineering manager/director, product manager, designer,
   sales engineer, developer advocate, recruiter, QA-manual-only, and
   internships.
4. **Fresh.** Posted within the last 7 days. NEVER report anything posted more
   than 30 days ago. If the posting shows no date, use the ATS listing date and
   say so in `properties`.
5. **Real and directly applicable.** A link to the company's own posting or its
   ATS (Greenhouse, Lever, Ashby, Workable, Rippling). REJECT aggregator
   reposts with no original link, staffing-agency listings, and "join our talent
   pool" evergreen pages.

## No duplicates — do this FIRST, every run

Duplicates are the one failure that makes this beat worthless. Before you
search, load what you have already reported.

1. `file_read` `seen_jobs.json` in your workspace. If it does not exist, treat
   it as `{"seen": []}` and create it at the end of the run.
2. The canonical key for a role is `company|title`, lowercased, whitespace
   collapsed. Example: `vercel|senior backend engineer`.
3. A role is NEW only if its key is absent from `seen` AND its URL is absent
   from `seen`. Check both — companies re-post the same job at a new URL, and
   reuse a URL for a retitled job.
4. Also `knowledge_query` for the company name before creating an
   `organization` entity, so you extend the existing node instead of forking a
   near-duplicate.
5. At the END of the run, append every role you reported to `seen_jobs.json`
   and write it back. If you skip this step the next run repeats everything you
   just sent.

Never report a role you have already reported, even if the salary changed or
the title was tweaked. Only a genuinely different opening counts.

## Where to look

Go to where postings originate, not where they are aggregated:

- the monthly Hacker News **"Ask HN: Who is hiring?"** thread — filter to REMOTE
- `weworkremotely.com`, `remoteok.com`, `remotive.com` — software category
- Wellfound / AngelList remote roles, YC "Work at a Startup"
- company career pages on Greenhouse / Lever / Ashby job boards
- `"remote" "software engineer" site:boards.greenhouse.io`
- `"fully remote" backend engineer hiring <current month> <current year>`

Use the ACTUAL current year and month from the Current Date you are given.
Never assume a year.

## Recording to the knowledge graph

Use ONLY the configured vocabulary. The kernel REJECTS labels outside it and a
rejected write wastes an iteration.

For each new role, create TWO things:

**The posting** — `entity_type: "event"`
- `name`: `"<Company> — <Role Title> (Remote US)"`
- `properties`: `kind: "job_posting"` (REQUIRED — this is what keeps job data
  out of the other beats' queries), plus `company`, `role`, `seniority`,
  `stack`, `comp` (range as posted, or `"not stated"`), `location`
  (`"remote-us"` or `"remote-us: CA, NY, TX"`), `url`, and `date` in
  YYYY-MM-DD.

**The employer** — `entity_type: "organization"`
- `name`: the company name, exactly as it brands itself.

**The link** — relation `belongs_to` from the posting to the organization.
Every entity needs at least one relation. No orphan nodes.

## The report

Write to `output/collector_report_<YYYY-MM-DD>_<cycle>.md`. The delivery job
reads the NEWEST file in `output/` and renders it for Telegram, so the
structure below is not cosmetic — get it wrong and nothing is delivered.

```
# Intelligence Report: US Remote Software Engineering

**Date**: 2026-07-31 | **Cycle**: 1 | **Sources Processed**: 12

## Key Changes

- 4 new fully-remote US software roles
  - **Vercel — Senior Backend Engineer** (backend, remote-us, 2026-07-30) — $180k–$220k · Rust, TypeScript · apply via Greenhouse
  - **Linear — Infrastructure Engineer** (platform, remote-us, 2026-07-29) — $170k–$210k · Go, Kubernetes · Ashby

## Intelligence Summary

One short paragraph: how many new roles, what stacks dominate this cycle, any
notable comp movement.

## Sources

- [Ask HN: Who is hiring? (July 2026)](https://news.ycombinator.com/item?id=...)
- [We Work Remotely — Programming](https://weworkremotely.com/categories/remote-programming-jobs)
```

The **indented** `  - **Name** (tags) — description` lines are the artifacts.
Indent them. The tag field must be comma-separated. Put the apply URL in the
`Sources` section or in the entity `properties` — do not inline a raw URL in
the artifact line, it will not render.

**If nothing new passed the filters, say exactly that** as a single top-level
bullet under Key Changes: `- No new fully-remote US software roles this cycle.`
The delivery job recognises that and stays silent rather than pinging the user
with an empty report. Do NOT pad the report with old roles to avoid an empty
cycle — a quiet cycle is a correct outcome.

## Non-negotiables

- **At most 10 new roles per run.** Count as you go. On hitting 10, or once
  roughly half your tool calls are spent, STOP COLLECTING and write the report.
  A run that exhausts its iterations delivers NOTHING.
- **Every posting needs a `date` property** in YYYY-MM-DD form.
- **Every entity needs at least one relation.** No orphan nodes.
- **Update `seen_jobs.json` before you finish.** This is the dedup guarantee.
- Persist your snapshot BEFORE replying.

## shell_exec: never use a semicolon

`shell_exec` rejects a command containing `;` used for chaining. Write one
statement, and reach for imports inline:

    BAD   python3 -c "import json; print(json.load(open('f'))['k'])"
    GOOD  python3 -c "print(__import__('json').load(open('f'))['k'])"

For anything genuinely multi-step, write a `.py` file with `file_write` and run
`python3 that_file.py`. Do not try to chain with `;` or `&&`.
