# Home, profiles and Explore: the private-first front page

**Status:** Implemented · **Date:** 2026-11-26
**Scope:** NicGit web app and read API · **Related:** decisions N56, N59, N60;
[repository navigation](2026-11-25-repository-navigation.md)

The front page is a dashboard for the signed-in person, not a project list. Profiles live at the
account name. Explore is the only public catalogue, and most projects are private by default.
Every noun follows the account's wording style (N61): the same layout reads as repositories and pull
requests for a Git user, or projects and proposals for an architectural practice.

---

## 1. Home

Three columns on a wide screen, one column on a phone:

| Column | Content |
|---|---|
| Left | **Repositories**: every project the person can read, ordered by last activity (newest first), with description, visibility, open proposal count and open issue count. A filter appears when there are more than four. |
| Centre | **Posts** from the person's notifications; then **Releases**, **Packages**, **Pull requests**, **Issues** and **Runs** across those repositories. |
| Right | **Change log**: recent NicGit and NicBim entries, switched by a two-tab control. **News**: short product notes. |

- Last activity is `max(activity.created_at)` for the project, falling back to `projects.created_at`.
  It never reads git directly, so the home stays cheap.
- A panel with no records shows one short empty state (`No runs yet.`), never fake data.
- The centre's Releases, Packages, Pull requests, Issues and Runs sit in a two-column grid; Posts
  stays full width.
- The changelog is curated next to the code (`web/src/lib/changelog.ts`): NicGit follows
  [CHANGELOG.md](../../CHANGELOG.md), NicBim follows its decision log until NicBim publishes a
  changelog of its own. It is not fetched and not editable in the app.
- News is a small static list in `web/src/lib/news.ts`, written with the release that ships it.
- The layout is in the existing trace-paper/ruled-schedule language (N55): ruled lists, `.tag` for
  status and `.btn` for actions, not cards.

### Contract

`GET /api/me/home` requires a session and returns:

| Key | Meaning |
|---|---|
| `repos` | Accessible projects, ordered by `last_activity_at`; includes `open_proposals`, `open_issues` |
| `posts` | The latest 8 notifications for this user |
| `releases` | The latest 8 milestones in accessible projects |
| `packages` | Empty until the package registry exists |
| `proposals` | The latest 8 open proposals |
| `issues` | The latest 8 open issues |
| `runs` | The latest 8 `checks` jobs |

Access is the same predicate as `/api/projects`: the owner account, organisation membership, direct
project grants and team grants. A project the caller cannot read is absent from every list.

## 2. Profiles

An account is addressed by its own name:

- `/ana` is Ana's profile.
- `/studio` is Studio Ferreira's organisation profile.

`/orgs/{slug}` redirects to `/{slug}`; the API keeps `/api/orgs/...`. There is no user-facing `/orgs`
prefix left.

A profile is public. It shows:

- avatar, display name, `@slug`, kind (Person or Organisation) and the caller's role in an org;
- projects owned by the account that the caller may read (public projects for anonymous visitors,
  plus private projects reached through membership or a grant);
- for a user viewing their own profile: **Edit profile** and **New project**;
- for an organisation member: Projects, Members and Teams; an admin also gets the management forms;
- for a user viewing their own profile: the organisations they belong to.

### Contract

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/accounts/{slug}` | Public account: id, slug, kind, display name |
| `GET` | `/api/accounts/{slug}/projects` | Projects of that account the caller may read |
| `GET` | `/api/orgs/{slug}` | Org role and, for members, the member list |
| `GET` | `/api/orgs/{slug}/teams` | Teams, for members |

Anonymous profile requests return only public projects. A private project remains absent and 404 on
every other route.

## 3. Explore

Explore lists only explicitly public projects. It has:

- a search box over project name, owner and description;
- an owner-kind filter: all, people, organisations;
- a sort order: newest, name, owner;
- a live result count and one empty state when nothing matches.

Filtering and sorting are client-side over the 100 projects the endpoint returns. The API is
unchanged. When the catalogue outgrows that limit, the same three controls become query parameters.

## 4. Private by default

Most projects are private: studio work, client work, competitions, internal models.

Public is the exception, used for university and coursework projects, public examples, demos and open
libraries. The product reflects that:

- `projects.visibility` defaults to `private`; the new-project form starts on Private.
- Explore never lists a private project, even for a signed-in member.
- Profiles show private projects only to callers who already have access.
- The home shows only accessible projects.
- Copy does not urge a person to publish a project.

Decision: N60.

## 5. Acceptance criteria

1. Home renders the three columns on a wide screen and one column on a phone; no horizontal page
   scroll outside tables.
2. Repositories are ordered by last activity, with the newest first.
3. Posts, Releases, Pull requests, Issues and Runs show real data or one short empty state.
4. Packages shows an empty state until the registry exists.
5. `/orgs/{slug}` redirects to `/{slug}`; every profile link uses the unprefixed URL.
6. A person can reach their own profile from the avatar menu.
7. Explore search, owner filter, sort and result count work together; clearing the search restores
   the list.
8. Anonymous callers see only public projects on a profile and in Explore.
9. The private-by-default rule is documented in the decision log and the new-project form still
   starts on Private.
