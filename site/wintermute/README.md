# mcphost.dev/wintermute

Static page served by Caddy at https://mcphost.dev/wintermute from
`mcphost-1:/var/www/mcphost/wintermute/` (route: mcphost-deploy `www_routes.render_wintermute_handles`).
Tracked here: `index.html` and `brand/*.svg`.

`live.json` is NOT page source. Carbon's `wintermute-live-export.timer` writes it every 2 min
(`~/.local/bin/wintermute-live-export.sh`). deploy.sh excludes it and never deletes anything.

Deploy, from a clean checkout whose HEAD is on origin/master:

    bash site/wintermute/deploy.sh --dry-run   # default; itemized rsync diff, writes nothing
    bash site/wintermute/deploy.sh --apply     # backup to /var/backups/mcphost-www, rsync, verify

It prints the backup path and a one-line restore command.
