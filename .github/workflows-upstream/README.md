# Workflows of the upstream project (switched off)

GitHub only runs files in `.github/workflows/`. These were moved here because they need things a
new repository doesn't have (self-hosted Mac/Linux runners, signing keys, `horizon-rust/horizon`
labels, schedules every 10 minutes) and would only fill the Actions tab with queued or failing runs.

To switch one on, move it back into `.github/workflows/` and give it what it needs.
The active workflow is `.github/workflows/build-exe.yml` (builds `POC404.exe`).
