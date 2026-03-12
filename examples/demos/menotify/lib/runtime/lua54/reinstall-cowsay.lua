local M = {}

M.doc = {
    name = "reinstall-cowsay",
    version = "0.1.0",
    author = "Sysinspect Demo",
    description = "Reinstall a package through PackageKit from Lua runtime.",
    arguments = {
        {
            name = "package",
            type = "string",
            required = false,
            description = "Package name to install back. Defaults to cowsay."
        }
    },
    returns = {
        description = "PackageKit installation result",
        sample = {
            requested = { "cowsay" },
            package_ids = { "cowsay;..." },
            changed = { "cowsay;..." }
        }
    }
}

--- Install the requested package through PackageKit.
-- @param req table Sysinspect runtime request.
-- @return table Result payload.
function M.run(req)
    local args = req.args or {}
    local pkg = args.package or "cowsay"

    if pkg == "" then
        error("argument 'package' must not be empty")
    end

    if not packagekit.available() then
        error("PackageKit is not available on this system")
    end

    log.info("Reinstalling package", pkg, "through PackageKit")
    return packagekit.install({ pkg })
end

return M
