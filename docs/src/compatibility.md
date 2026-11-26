# Compatibility and API limits

plane-cli is tested on **Plane Community Edition 1.4.2**. Other versions may work, but they are not tested. If something breaks on your version, please open a [Plane compatibility issue](https://github.com/pseudovar/plane-cli/issues/new?template=plane-compat.yml).

## Limits of the Plane public API

These come from Plane CE 1.4.2 and affect what plane-cli can do:

| Area | Limit |
|------|-------|
| Relations | Can be created and listed, but the public API has no way to remove one. Use the Plane UI |
| Notifications | No public endpoint, so `inbox` cannot include @mentions |
| Assignee filtering | No server-side filter. plane-cli lists and filters locally, up to 5,000 work items per project |
| Project members | Update and remove need the membership id that `projects members add` returned. Members added in the UI cannot be changed through the API, and a removed member cannot be re-added through it |
| Modules | Only completed or cancelled modules can be archived |
| Cycles | A work item can be in one cycle only, so adding it moves it. `cycles transfer` works only from a completed cycle and moves only unfinished items. Start and end dates are both set or both empty |
| States | CE ignores `sequence` on create, and does not clear the previous default. plane-cli applies both with a follow-up update and warns |
| Archived projects | Stay in `projects list`, but their work items, states, cycles and modules read as empty lists and single gets return 404 until unarchived |
| Intake | Pending intake items are read with `intake get`, not `work-items get` |

`invitations update` follows the API schema but has not been tested against a live pending invitation.

## Platforms

Release binaries are built for Linux x86_64 (glibc 2.35 or newer, from the `ubuntu-22.04` runner), macOS arm64 and macOS x86_64. Other platforms can build from source.
