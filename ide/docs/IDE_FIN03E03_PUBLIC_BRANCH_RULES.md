# FIN03-6E03 — GitHub base branch protection (public read only)

A registered project must have an exactly scoped, same-HEAD and same-origin
open PR. The native host derives the base branch and canonical GitHub
repository, performs bounded unauthenticated HTTPS with redirects disabled,
parses only allowlisted public rule fields, then rechecks the same local,
PR-head and PR-base SHA. Missing public access (401/403/404) is unknown,
**not proof of an unprotected branch**.

This endpoint does not cover all GitHub rulesets, bypass actors, deploy
environments, admin exemptions, private policies or current mergeability.
It always reports `rulesetsCovered:false`, `protectionComplete:false`
and `mergeAuthorized:false`. No credentials or remote write operations
are introduced. Exact-head CI must be separately checked and cannot be
considered current after source drift.

Native compiler, security, online response and installed acceptance
remain deferred.
