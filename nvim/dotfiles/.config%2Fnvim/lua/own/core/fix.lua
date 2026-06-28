local M = {}

function M.set_qflist(title, command)
    local lines = vim.fn.systemlist(command)
    vim.fn.setqflist({}, 'r', { title = title, lines = lines })
    vim.cmd('copen')
end

function M.set_loclist(title, command)
    local lines = vim.fn.systemlist(command)
    vim.fn.setloclist(0, {}, 'r', { title = title, lines = lines })
    vim.cmd('lopen')
end

local language_files = vim.api.nvim_get_runtime_file('lua/own/core/fix-languages/*.lua', true)
for _, language_file in ipairs(language_files) do
    local language = language_file:match(".+/([^/]+)%.lua")
    require('own.core.fix-languages.' .. language)(M.set_qflist, M.set_loclist)
end

return M
