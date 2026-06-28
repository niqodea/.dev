local M = {}

function M.get_atdir()
    local atdir = vim.fn.getenv('ATDIR')
    if atdir == vim.NIL or atdir == '' then
        error('ATDIR env variable is not set')
    end
    return atdir
end

return M
