local M = {}

function M.get_atdir()
    local atdir = vim.fn.getenv('DOTDEV_ATDIR')
    if atdir == vim.NIL or atdir == '' then error('DOTDEV_ATDIR env variable is not set') end
    return atdir
end

return M
