MeNotify Demo Sensors
=====================

This demo ships two real `menotify` Lua sensors:

- `menotify.githubissues`
- `menotify.pkgnotify`

- `githubissues` polls issues on a public GitHub repository and emits one event
  per newly opened issue. The first poll only seeds the local cursor and emits
  nothing.
- `pkgnotify` polls the installed-package snapshot through PackageKit and emits
  one event when watched packages disappear. In this demo it watches `cowsay`
  and immediately triggers a model action that installs `cowsay` back.

What this demo contains
-----------------------

- `sensors.cfg`
  Sensor definition and event handlers.
- `model.cfg`
  Self-heal model that restores a removed package through Lua runtime.
- `lib/sensors/lua/githubissues.lua`
  GitHub issues polling sensor.
- `lib/sensors/lua/pkgnotify.lua`
  PackageKit installed-package snapshot polling sensor.
- `lib/runtime/lua/packagekit.lua`
  Generic Lua runtime action that runs PackageKit install/remove/upgrade operations.

Master
------

1. Copy `model.cfg` to the master's models root into subdirectory
   `menotify`, so you end up with:

      `$MASTER/data/models/menotify/model.cfg`

2. Copy `sensors.cfg` to the master's sensors root into subdirectory
   `menotify`, so you end up with:

      `$MASTER/data/sensors/menotify/sensors.cfg`

3. From this demo directory, publish the Lua sensor library tree:

      `sysinspect module -A --path ./lib -l`

   This uploads:

   - `lib/sensors/lua/githubissues.lua`
   - `lib/sensors/lua/pkgnotify.lua`
   - `lib/runtime/lua/packagekit.lua`

4. Edit master config so both the model and sensor scopes are exported:

   ```yaml
   config:
     master:
       fileserver.models:
         - menotify
       fileserver.sensors:
         - menotify
   ```

5. Sync the cluster:

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

Configure PackageKit self-heal
------------------------------

The `packagekit-history` sensor is already wired to:

- watch only `cowsay`
- notice only `removed`
- call model query `menotify/tracked-package`

The model action runs `runtime.lua-runtime` with:

- `rt.mod: packagekit`
- `action: context(action)`
- `package: context(package)`

So the full path is:

1. `menotify.pkgnotify` notices that `cowsay` disappeared.
2. The sensor emits `removed`.
3. The `pipeline` handler passes `action: install` and `package: $.data.package`.
4. The demo model action receives that context and forwards it into runtime args.
5. `runtime.lua-runtime` runs `lib/runtime/lua/packagekit.lua`.
6. That generic Lua runtime script calls `packagekit.install({ "cowsay" })`.

Query syntax note:

- model queries are `<model>/[entity]/[state]`
- `package-op` is an action id inside the model, not a query path component

What to expect from PackageKit
------------------------------

1. Start the minion on a Linux system with PackageKit available.
2. Wait for the first polling cycle.
3. The first poll seeds the local PackageKit snapshot and emits nothing.
4. Remove `cowsay`.
5. On the next poll, the Lua sensor logs:

      `Package cowsay was removed`

6. The sensor emits a normal Sysinspect event with action:

    - `removed`

7. The `pipeline` handler invokes the demo model action with `action=install`
   and `package=cowsay`.
8. The Lua runtime remediation script logs a PackageKit install operation.

9. PackageKit installs `cowsay` back.

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
- The PackageKit self-heal demo is Linux-only by design and simply does nothing
  if PackageKit is unavailable.
