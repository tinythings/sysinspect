MeNotify Demo Sensors
=====================

This demo ships two real `menotify` Lua sensors:

- `menotify.githubissues`
- `menotify.pkgnotify`

- `githubissues` polls issues on a public GitHub repository and emits one event
  per newly opened issue. The first poll only seeds the local cursor and emits
  nothing.
- `pkgnotify` polls PackageKit history and emits one event per newly seen
  package install or removal for the configured package list.

What this demo contains
-----------------------

- `sensors.cfg`
  Sensor definition and event handlers.
- `model.cfg`
  Minimal placeholder model to keep the demo layout consistent.
- `lib/sensors/lua/githubissues.lua`
  GitHub issues polling sensor.
- `lib/sensors/lua/pkgnotify.lua`
  PackageKit history polling sensor.

Master
------

1. Copy `sensors.cfg` to the master's sensors root into subdirectory
   `menotify`, so you end up with:

      `$MASTER/data/sensors/menotify/sensors.cfg`

2. From this demo directory, publish the Lua sensor library tree:

      `sysinspect module -A --path ./lib -l`

   This uploads:

   - `lib/sensors/lua/githubissues.lua`
   - `lib/sensors/lua/pkgnotify.lua`

3. Edit master config so this sensor scope is exported:

   ```yaml
   config:
     master:
       fileserver.sensors:
         - menotify
   ```

4. Sync the cluster:

      `sysinspect --sync`

Minion
------

Nothing special. Just let it autosync and restart the minion after sensor
configuration changes so the listener is reloaded.

Configure the repository
------------------------

Edit `sensors.cfg` and set:

- `owner`
- `repo`

Use a public repository you control, so you can keep opening test issues.

Optional args already supported by the script:

- `state`
- `per_page`
- `user_agent`
- `token`
- `api`
- `bootstrap_emit_existing`

Configure PackageKit polling
----------------------------

Edit `sensors.cfg` and set the package list for `packagekit-history`:

- `packages`
- `history_count`
- `bootstrap_emit_existing`

Example:

```yaml
packagekit-history:
  listener: menotify.pkgnotify
  args:
    packages:
      - bash
      - openssl
    history_count: 20
    bootstrap_emit_existing: false
```

What to expect from PackageKit
------------------------------

1. Start the minion on a Linux system with PackageKit available.
2. Wait for the first polling cycle.
3. The first poll seeds the local PackageKit snapshot and emits nothing.
4. Install a watched package.
5. On the next poll, the Lua sensor logs:

      `Package <name> was installed`

6. Remove the same package.
7. On the next poll, the Lua sensor logs:

      `Package <name> was removed`

8. The sensor also emits normal Sysinspect events with action:

- `installed`
- `removed`

What to expect
--------------

1. Start the minion.
2. Wait for the first polling cycle.
3. The first poll seeds the local issue cursor and emits nothing.
4. Create a new issue in the configured GitHub repository.
5. On the next poll, the Lua sensor logs:

      `New issue here: #<number> <title>`

6. The sensor also emits a normal Sysinspect event. `console-logger` prints the
   payload on the minion.

Notes
-----

- Pull requests are ignored.
- This example is intentionally public-repo friendly and does not require a
  token.
- If you do pass `token` in `args`, the script sends it as a bearer token.
- The PackageKit demo is Linux-only by design and simply does nothing if
  PackageKit is unavailable.
