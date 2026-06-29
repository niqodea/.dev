require('x.core')

vim.cmd('colorscheme retrobox')

-- Extract module names from files
local module_files = vim.api.nvim_get_runtime_file('lua/x/modules/*.lua', true)
local modules = {}
for _, module_file in ipairs(module_files) do
    local module = module_file:match(".+/([^/]+)%.lua")
    table.insert(modules, module)
end

-- Create load commands for modules
for _, module in ipairs(modules) do
    local capitalized_module = module:gsub('^%l', string.upper)
    local load_command = 'XLoad' .. capitalized_module
    local module_path = 'x.modules.' .. module
    vim.api.nvim_create_user_command(load_command, function()
        require(module_path)
    end, {})
end

-- Startup logic for automatic loading of modules
local atdir = require('x.core.utils').get_atdir()
local auto_path = atdir .. '/auto.nvim-x'

vim.api.nvim_create_user_command('XAuto', function()
    vim.cmd('edit ' .. auto_path)
    if vim.fn.filereadable(auto_path) == 0 then
        local lines = {}
        for _, module in ipairs(modules) do
            table.insert(lines, '# ' .. module)
        end
        vim.api.nvim_buf_set_lines(0, 0, -1, false, lines)
    end
end, {})

local M = {}

function M.load_auto_modules()
    if vim.fn.filereadable(auto_path) == 0 then
        return
    end
    local auto_file = io.open(auto_path, 'r')
    for line in auto_file:lines() do
        if line:match('^%s*#') or line:match('^%s*$') then
            goto continue
        end
        local module = line:match('^%s*(%S+)')
        local module_path = 'x.modules.' .. module
        require(module_path)
        ::continue::
    end
    auto_file:close()
end

return M
