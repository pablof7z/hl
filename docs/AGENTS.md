# AGENTS.md — Highlighter Documentation

> This directory contains the canonical product specifications, architecture docs, market research, and wireframes for the Highlighter project. These documents are the source of truth for product decisions and technical implementation.

## Document Index

| File | Content | Status |
|---|---|---|
| `client-spec-v1.0.md` | **Authoritative** client-side spec. Navigation, screens, design system, components, interactions. | Current |
| `product-spec-v2.0.md` | **Authoritative** product spec. Core concepts, features, growth loops, platform architecture. | Current |
| `product-spec-v1.2.md` | Previous spec version. Historical reference only — superseded by v2.0. | Superseded |
| `technical-architecture.md` | System architecture, NIP mapping, data models, relay design, client design. | Current |
| `market-research-2026.md` | Market research, competitor analysis, user pain points, positioning, quotable UGC. | Current |
| `community-page-proposals-v1.4.md` | Community/shelf page wireframes (latest version). | Current |
| `community-page-proposals.md` | Community page wireframes (v1). Superseded by v1.4. | Superseded |
| `landing-page-proposals.md` | Landing page wireframes and proposals. | Current |

## Conventions

- **Always link to the current version** when referencing a doc. Don't link to superseded versions unless making a historical comparison.
- **Product decisions**: `product-spec-v2.0.md` is the single source of truth. If code conflicts with the spec, the spec wins until explicitly changed.
- **Architecture decisions**: `technical-architecture.md` is the authority on NIP mapping, event kinds, relay design, and client structure.
- **Wireframes** describe intended UI behavior. They are proposals, not pixel-perfect specs — implementation can adjust layout and interaction details.

## When Adding New Documents

1. **Version your files** — use a version suffix (e.g., `-v2.0.md`) when replacing a previous version
2. **Mark the old version as superseded** in this index
3. **Keep filenames descriptive** — `product-spec-v2.0.md`, not `spec-new.md`
4. **Update this AGENTS.md** with the new document's filename, description, and status

## Editing Guidelines

- Use **Markdown** for all docs
- Start each doc with a `# Title` and `## Version X.Y | Date` header
- Use **tables** for structured comparisons (NIP mapping, feature matrices, group types)
- Use **code blocks** with `jsonc` for event schemas and protocol examples
- Use **ASCII diagrams** for architecture flows (see `technical-architecture.md` for examples)
- Keep **diagrams text-based** — no binary images in docs. If you need visuals, describe them or use ASCII.

## Key Cross-References

- **Client spec** is the authority on navigation, screens, design system, and interaction patterns
- **Client spec** references **product spec** for core concepts and **product surfaces** for technical stacks
- **Product spec** references **technical architecture** for NIP-29 mapping and event kind definitions
- **Technical architecture** references **product spec** for feature scope (MVP vs. post-MVP)
- **Market research** informs **product spec** positioning and growth loops
- **Client spec** supersedes all prior wireframe and proposal documents for screen-level definitions

## Terms & Definitions

| Term | Definition |
|---|---|
| **Artifact** | A piece of external content shared to a group (book, article, podcast, video, etc.) |
| **Highlight** | An excerpt pulled from an artifact by a member |
| **Group** | A NIP-29 relay-based community — the core organizational unit |
| **Discussion** | Threaded comments on artifacts or highlights (NIP-22) |
| **Closed group** | Invite-only membership (NIP-29 `closed` tag) |
| **Private group** | Members-only reading (NIP-29 `private` tag) |
| **Restricted group** | Only members can write (always set in Highlighter) |
| **Vault** | Personal collection of a user's highlights across all groups |