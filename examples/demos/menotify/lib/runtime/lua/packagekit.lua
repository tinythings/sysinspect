local M = {}

M.doc = {
    name = "packagekit",
    version = "0.1.0",
    author = "Sysinspect Demo",
    description = "Generic PackageKit runtime helper for install, remove, and upgrade operations.",
    arguments = {
        {
            name = "action",
            type = "string",
            required = true,
            description = "Requested PackageKit operation: install, remove, or upgrade."
        },
        {
            name = "package",
            type = "string",
            required = false,
            description = "Single package name to operate on."
        },
        {
            name = "packages",
            type = "array",
            required = false,
            description = "Package names to operate on."
        }
    },
    returns = {
        description = "PackageKit operation result",
        sample = {
            action = "install",
            requested = { "cowsay" },
            package_ids = { "cowsay;..." },
            changed = { "cowsay;..." }
        }
    }
}

--- Return the requested package list as an array.
-- @param req table Sysinspect runtime request.
-- @return table Array of package names.
local function packages(req)
    if type(req.args.packages) == "table" then
        local out = {}
        for _, name in ipairs(req.args.packages) do
            if tostring(name) ~= "" then
                out[#out + 1] = tostring(name)
            end
        end
        return out
    end

    if type(req.args.package) == "string" and req.args.package ~= "" then
        return { req.args.package }
    end

    error("argument 'package' or 'packages' must be defined")
end

--- Run the requested generic PackageKit operation.
-- @param req table Sysinspect runtime request.
-- @return table Result payload.
function M.run(req)
    local action = tostring((req.args or {}).action or "")

    if action == "" then
        error("argument 'action' must be defined")
    end

    if not packagekit.available() then
        error("PackageKit is not available on this system")
    end

    if action == "install" then
        return packagekit.install(packages(req))
    end

    if action == "remove" then
        return packagekit.remove(packages(req))
    end

    if action == "upgrade" then
        return packagekit.upgrade(packages(req))
    end

    error("unsupported PackageKit action: " .. action)
end

return M
