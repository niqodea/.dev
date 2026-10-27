local M = {}

function M.get_atdir()
    local atdir = vim.env.DOTDEV_ATDIR
    if atdir == nil or atdir == '' then error('DOTDEV_ATDIR env variable is not set') end
    return atdir
end

return M
