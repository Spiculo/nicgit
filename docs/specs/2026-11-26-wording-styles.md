# Wording styles: Git conventions or architectural conventions

**Status:** Implemented · **Date:** 2026-11-26
**Scope:** NicGit web app and account settings · **Related:** decisions N60, N61;
[home, profiles and Explore](2026-11-26-home-profiles-explore.md)

The same product has two audiences. A studio that already uses Git understands *repository, pull
request, issue, branch and CI*. An architectural practice thinks in *project, proposal, task, option
and workflow*. Each account chooses the wording it reads; the data model, URLs and API do not change.

---

## 1. The setting

- Stored on the user account (`users.style`), default `git`.
- Changed in **Settings → Language & wording → Wording**.
- `GET /api/me` returns `style`; `PATCH /api/me {"style": "git" | "architecture"}` updates it.
- It is per user, like the language. Colleagues in the same organisation can read the interface
  differently.

## 2. Terms

| Git style (en / es) | Architectural style (en / es) |
|---|---|
| Repository / Repositorio | Project / Proyecto |
| Pull request / Solicitud | Proposal / Propuesta |
| Issue / Incidencia | Task / Tarea |
| Branch / Rama | Option / Opción |
| CI / CI | Workflow / Flujo de trabajo |
| Package / Paquete | Library / Biblioteca |
| Release / Publicación | Milestone / Hito |
| Tag / Etiqueta | Snapshot / Marca de versión |
| Push / Subir | Upload / Cargar |
| Head / Cabeza | Latest version / Última versión |
| Code tab / Código | Project tab / Proyecto |
| Configs / Configuración | Settings / Ajustes |

The surrounding noun stays the same: a project has *versions*, *design options*, *proposals* and
*releases-as-milestones* in both styles. "Combine", "version" and "design option" are domain words,
not Git words.

## 3. Implementation

- `web/src/i18n/en.ts` and `es.ts` are the base dictionaries; they use Git conventions.
- `web/src/i18n/styles.ts` holds the architectural overlay for English and Spanish. Only the keys
  that differ are listed; everything else falls back to the base.
- `translate(lang, style, key)` resolves style overlay → language → English → key. `useT()` reads both
  `LangContext` and `StyleContext`.
- The whole UI reads through `useT()`, so switching the style re-renders every label without a
  reload.
- The first-run welcome ([first run](2026-11-26-first-run.md)) asks for the style before the app
  opens; Settings → Language & wording changes it later.
- A Vitest test checks that every architectural string keeps the placeholders of its base string.
- Route paths (`/branches`, `/issues`, `/proposals`), API paths and database names stay as they are;
  the wording layer is presentation only.

## 4. Acceptance criteria

1. A new account starts on the Git style.
2. Settings → Language & wording shows Git and Architectural, with the active one marked.
3. Switching the style changes the project tabs, Home headings, page titles, empty states and activity
   lines immediately.
4. Switching the language keeps the chosen style.
5. Both styles have English and Spanish strings; no label falls back to a raw key.
6. Git style uses repository, pull request, issue, branch, CI, package, release and tag.
7. Architectural style uses project, proposal, task, option, workflow, library, milestone and
   snapshot.
8. The setting is per user and does not change another account's interface.
